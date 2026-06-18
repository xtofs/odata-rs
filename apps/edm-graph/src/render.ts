import * as d3 from "d3";
import type { Graph, GraphEdge, GraphNode, GraphPath, SimEdge } from "./types";

const NODE_RADIUS = 9;
const ARROW_PADDING = NODE_RADIUS + 6; // keep arrowheads off the node circle
const MARKER_OFFSET = NODE_RADIUS + 9; // push order badges clear of the node

/**
 * Render the whole EDM graph: force-directed nodes/edges plus per-path
 * polylines, wired up with hover highlighting from the canvas and the panel.
 */
export function renderGraph(graph: Graph, svgEl: SVGSVGElement, panelEl: HTMLElement): void {
  const svg = d3.select(svgEl);
  const width = svgEl.clientWidth || 800;
  const height = svgEl.clientHeight || 600;
  svg.attr("viewBox", `0 0 ${width} ${height}`);

  // d3.forceLink mutates edges in place, turning source/target ids into nodes.
  const edges = graph.edges as unknown as SimEdge[];
  const nodes = graph.nodes;
  const paths = graph.paths;

  // ----- Color scale: one stable color per path id. -----
  const color = d3
    .scaleOrdinal<string, string>(d3.schemeTableau10)
    .domain(paths.map((p) => p.id));

  // ----- Lookups for fast hover highlighting. -----
  const nodeById = new Map(nodes.map((n) => [n.id, n]));
  const edgesByNode = new Map<string, Set<string>>(); // nodeId -> edge ids
  const pathsByNode = new Map<string, Set<string>>(); // nodeId -> path ids

  for (const e of graph.edges) {
    addTo(edgesByNode, e.source, e.id);
    addTo(edgesByNode, e.target, e.id);
  }
  for (const p of paths) {
    for (const seg of p.segments) {
      addTo(pathsByNode, seg.node, p.id);
    }
  }

  // ----- Expand/collapse structure. -----
  // A node is a "parent" if it has outgoing edges of these kinds; its targets
  // are its children. This covers EntityTypes, ComplexTypes, Functions,
  // Actions, EntityContainers, EnumTypes, Schemas — anything that declares
  // children — without hard-coding kinds.
  const CHILD_EDGE_KINDS = new Set(["Declares", "Contains"]);
  const childrenOf = new Map<string, string[]>(); // parent id -> child ids
  const childIds = new Set<string>(); // every node that is some parent's child

  for (const e of graph.edges) {
    if (!CHILD_EDGE_KINDS.has(e.kind)) continue;
    const list = childrenOf.get(e.source) ?? [];
    list.push(e.target);
    childrenOf.set(e.source, list);
    childIds.add(e.target);
  }

  // Initialize per-node expand/collapse state (default collapsed).
  for (const n of nodes) {
    n.expanded = false;
    n.children = childrenOf.get(n.id) ?? [];
  }

  const isExpandable = (id: string): boolean => (childrenOf.get(id)?.length ?? 0) > 0;

  // ----- Defs: arrowhead marker. -----
  const defs = svg.append("defs");
  defs
    .append("marker")
    .attr("id", "arrow")
    .attr("viewBox", "0 -5 10 10")
    .attr("refX", 10)
    .attr("refY", 0)
    .attr("markerWidth", 7)
    .attr("markerHeight", 7)
    .attr("orient", "auto-start-reverse")
    .append("path")
    .attr("d", "M0,-5L10,0L0,5")
    .attr("fill", "var(--edge)");

  // ----- Draw-order groups (bottom to top). -----
  // Structural edges form the base layer. Path overlays and their order
  // markers sit ON TOP of the edges. Nodes sit on top of everything. The
  // overlay layers must not capture pointer events, so node hover/drag still
  // works through them.
  // All layers live inside a single viewport group that the zoom behavior
  // transforms, so the whole graph can be zoomed/panned together.
  const gRoot = svg.append("g").attr("class", "viewport");
  const gEdges = gRoot.append("g").attr("class", "edges");
  const gPaths = gRoot.append("g").attr("class", "paths").attr("pointer-events", "none");
  // Edge labels sit above the edges/overlays but below the nodes.
  const gEdgeLabels = gRoot.append("g").attr("class", "edge-labels").attr("pointer-events", "none");
  const gNodes = gRoot.append("g").attr("class", "nodes");
  // Order markers sit above the nodes so the numbered badges stay readable.
  const gMarkers = gRoot.append("g").attr("class", "markers").attr("pointer-events", "none");

  // ----- Zoom / pan. Drag on a node moves the node (d3-drag consumes the
  // gesture); dragging empty canvas pans; wheel zooms. -----
  const zoom = d3
    .zoom<SVGSVGElement, unknown>()
    .scaleExtent([0.2, 4])
    .on("zoom", (event) => gRoot.attr("transform", event.transform.toString()));
  svg.call(zoom);

  // ----- Edges as curved paths with arrowheads. -----
  const edgeSel = gEdges
    .selectAll<SVGPathElement, SimEdge>("path.edge")
    .data(edges, (d) => d.id)
    .join("path")
    .attr("class", "edge")
    .attr("fill", "none")
    .attr("stroke", "var(--edge)")
    .attr("stroke-width", 1.5)
    .attr("marker-end", "url(#arrow)");

  // Edge labels: "has" for parent/child (Declares/Contains) edges, otherwise
  // the edge's own attribute name from the JSON.
  const edgeLabel = (e: SimEdge): string =>
    CHILD_EDGE_KINDS.has(e.kind) ? "has" : e.label;

  const edgeLabelSel = gEdgeLabels
    .selectAll<SVGTextElement, SimEdge>("text.edge-label")
    .data(edges, (d) => d.id)
    .join("text")
    .attr("class", "edge-label")
    .attr("text-anchor", "middle")
    .attr("dy", "-0.25em")
    .attr("font-size", "10px")
    .attr("fill", "var(--muted)")
    .attr("stroke", "var(--bg)")
    .attr("stroke-width", 3)
    .attr("paint-order", "stroke")
    .text(edgeLabel);

  // ----- Path polylines (one per path) + numbered order markers. -----
  const line = d3
    .line<GraphNode>()
    .x((d) => d.x ?? 0)
    .y((d) => d.y ?? 0)
    // Linear so the overlay sits directly on top of the straight structural
    // edges connecting the same node centers.
    .curve(d3.curveLinear);

  // Position of each path in the list, used to fan order badges out around a
  // node so paths sharing a node don't stack their badges on the same spot.
  const pathIndex = new Map(paths.map((p, i) => [p.id, i]));

  // Resolve each path's ordered node list once.
  const pathNodes = new Map<string, GraphNode[]>();
  for (const p of paths) {
    const ordered = [...p.segments]
      .sort((a, b) => a.order - b.order)
      .map((s) => nodeById.get(s.node))
      .filter((n): n is GraphNode => n !== undefined);
    pathNodes.set(p.id, ordered);
  }

  // Which paths are currently revealed. Empty = nothing selected, so no path
  // colors or order numbers are drawn at all.
  const activePaths = new Set<string>();
  // Node ids currently visible per the expand/collapse state.
  let visibleNodes = new Set<string>();
  // Persistent selection: the path pinned by clicking its row (null = none).
  let selectedPath: string | null = null;
  // Tracks the Space-bar expand-all / collapse-all toggle.
  let allExpanded = false;

  const pathSel = gPaths
    .selectAll<SVGPathElement, GraphPath>("path.path-line")
    .data(paths, (d) => d.id)
    .join("path")
    .attr("class", "path-line")
    .attr("fill", "none")
    .attr("stroke", (d) => color(d.id))
    .attr("stroke-width", 3)
    .attr("stroke-linejoin", "round")
    .attr("stroke-linecap", "round")
    .attr("opacity", 0.85);

  // Flatten segments into per-marker data so we can render numbered badges.
  interface MarkerDatum {
    pathId: string;
    node: GraphNode;
    order: number;
  }
  const markerData: MarkerDatum[] = [];
  for (const p of paths) {
    for (const seg of p.segments) {
      const node = nodeById.get(seg.node);
      if (node) markerData.push({ pathId: p.id, node, order: seg.order });
    }
  }

  const markerSel = gMarkers
    .selectAll<SVGGElement, MarkerDatum>("g.order-marker")
    .data(markerData)
    .join("g")
    .attr("class", "order-marker");

  markerSel
    .append("circle")
    .attr("r", 7)
    .attr("fill", (d) => color(d.pathId))
    .attr("stroke", "#0f1115")
    .attr("stroke-width", 1.5);

  markerSel
    .append("text")
    .text((d) => String(d.order))
    .attr("text-anchor", "middle")
    .attr("dy", "0.35em")
    .attr("font-size", "9px")
    .attr("font-weight", 700)
    .attr("fill", "#0f1115")
    .attr("pointer-events", "none");

  // ----- Nodes: group with circle + label. -----
  const nodeSel = gNodes
    .selectAll<SVGGElement, GraphNode>("g.node")
    .data(nodes, (d) => d.id)
    .join("g")
    .attr("class", "node");

  nodeSel
    .append("circle")
    .attr("r", NODE_RADIUS)
    .attr("fill", "var(--node)")
    .attr("stroke", "var(--node-stroke)")
    .attr("stroke-width", 1.5);

  nodeSel
    .append("text")
    .text((d) => d.label)
    .attr("x", NODE_RADIUS + 4)
    .attr("dy", "0.35em")
    .attr("font-size", "11px")
    .attr("fill", "var(--text)")
    .attr("pointer-events", "none");

  // Expand/collapse indicator, placed to the left of expandable nodes.
  nodeSel
    .append("text")
    .attr("class", "toggle")
    .attr("x", -(NODE_RADIUS + 6))
    .attr("text-anchor", "middle")
    .attr("dy", "0.35em")
    .attr("font-size", "14px")
    .attr("font-weight", 700)
    .attr("fill", "var(--node-stroke)")
    .attr("pointer-events", "none")
    .text("");

  // ----- Force simulation. -----
  // Initialized over the full graph once so every node gets a starting
  // position and every edge's source/target string is resolved to a node
  // object. applyVisibility() then narrows the active node/link sets.
  const linkForce = d3
    .forceLink<GraphNode, GraphEdge>(graph.edges)
    .id((d) => d.id)
    .distance(120);

  const simulation = d3
    .forceSimulation<GraphNode>(nodes)
    .force("link", linkForce)
    .force("charge", d3.forceManyBody().strength(-400))
    .force("center", d3.forceCenter(width / 2, height / 2))
    .force("collide", d3.forceCollide<GraphNode>(NODE_RADIUS + 14));

  // ----- Tick: recompute every geometry-dependent attribute. -----
  simulation.on("tick", () => {
    edgeSel.attr("d", edgePath);
    edgeLabelSel
      .attr("x", (d) => ((d.source.x ?? 0) + (d.target.x ?? 0)) / 2)
      .attr("y", (d) => ((d.source.y ?? 0) + (d.target.y ?? 0)) / 2);
    nodeSel.attr("transform", (d) => `translate(${d.x ?? 0},${d.y ?? 0})`);

    // Path polylines must recompute from current node coordinates each tick.
    pathSel.attr("d", (d) => line(pathNodes.get(d.id) ?? []));

    markerSel.attr("transform", (d) => {
      // Start the fan at the top (−90°) so badges sit above/below the node,
      // clear of the right-aligned node labels.
      const angle =
        -Math.PI / 2 + ((pathIndex.get(d.pathId) ?? 0) / Math.max(paths.length, 1)) * 2 * Math.PI;
      const ox = Math.cos(angle) * MARKER_OFFSET;
      const oy = Math.sin(angle) * MARKER_OFFSET;
      return `translate(${(d.node.x ?? 0) + ox},${(d.node.y ?? 0) + oy})`;
    });
  });

  // ----- Drag behavior. -----
  // `didDrag` distinguishes a drag from a click so dragging a node doesn't
  // also toggle its expand/collapse state.
  let didDrag = false;
  nodeSel.call(
    d3
      .drag<SVGGElement, GraphNode>()
      .on("start", (event, d) => {
        didDrag = false;
        if (!event.active) simulation.alphaTarget(0.3).restart();
        d.fx = d.x;
        d.fy = d.y;
      })
      .on("drag", (event, d) => {
        didDrag = true;
        d.fx = event.x;
        d.fy = event.y;
      })
      .on("end", (event, d) => {
        if (!event.active) simulation.alphaTarget(0);
        d.fx = null;
        d.fy = null;
      })
  );

  // ----- Click a node to expand/collapse its children. -----
  nodeSel.on("click", (_event, d) => {
    if (didDrag) {
      didDrag = false;
      return;
    }
    if (!isExpandable(d.id)) return;
    d.expanded = !d.expanded;
    applyVisibility();
  });

  // ----- Hover preview from the canvas (transient; reverts to selection). ----
  nodeSel
    .on("mouseenter", (_event, d) => highlightNode(d.id))
    .on("mouseleave", () => applySelection());

  // ----- Keyboard: Space toggles expand-all / collapse-all. -----
  window.addEventListener("keydown", (event) => {
    const target = event.target as HTMLElement | null;
    const typing = target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName);
    if (event.code !== "Space" || typing) return;
    event.preventDefault();
    allExpanded = !allExpanded;
    for (const n of nodes) {
      if (isExpandable(n.id)) n.expanded = allExpanded;
    }
    applyVisibility();
  });

  function highlightNode(nodeId: string): void {
    const incidentEdges = edgesByNode.get(nodeId) ?? new Set();
    const incidentPaths = pathsByNode.get(nodeId) ?? new Set();

    // Neighboring nodes via incident edges, plus the node itself.
    const liveNodes = new Set<string>([nodeId]);
    for (const e of graph.edges) {
      if (incidentEdges.has(e.id)) {
        liveNodes.add(e.source);
        liveNodes.add(e.target);
      }
    }

    applyHighlight({
      nodes: liveNodes,
      edges: incidentEdges,
      paths: incidentPaths
    });
  }

  // ----- Side panel: path list + hover highlighting. -----
  buildPanel();

  // Apply the initial (all-collapsed) visibility state.
  applyVisibility();

  function buildPanel(): void {
    const sel = d3
      .select(panelEl)
      .selectAll<HTMLDivElement, GraphPath>("div.path-row")
      .data(paths, (d) => d.id)
      .join("div")
      .attr("class", "path-row")
      .on("click", (_event, d) => selectPath(selectedPath === d.id ? null : d.id));

    sel
      .append("span")
      .attr("class", "path-swatch")
      .style("background", (d) => color(d.id));

    const meta = sel.append("span").attr("class", "path-meta");
    meta
      .append("div")
      .attr("class", "path-label")
      .text((d) => d.label);
    meta
      .append("div")
      .attr("class", "path-owner")
      .text((d) => d.owner);
  }

  // ----- Click a path row to show its metadata. -----
  const detailsEl = document.getElementById("path-details");

  function showMetadata(path: GraphPath): void {
    if (!detailsEl) return;

    const steps = [...path.segments]
      .sort((a, b) => a.order - b.order)
      .map((s) => {
        const node = nodeById.get(s.node);
        const label = node ? node.label : s.node;
        const kind = node ? node.kind : "unknown";
        return `<li><span class="pd-order">${s.order}</span><span class="pd-node">${escapeHtml(
          label
        )}</span><span class="pd-kind">${escapeHtml(kind)}</span></li>`;
      })
      .join("");

    detailsEl.innerHTML = `
      <div class="pd-head">
        <span class="pd-swatch" style="background:${color(path.id)}"></span>
        <span class="pd-title">${escapeHtml(path.label)}</span>
      </div>
      <div class="pd-meta"><span>owner</span>${escapeHtml(path.owner)}</div>
      <div class="pd-meta"><span>id</span>${escapeHtml(path.id)}</div>
      <ol class="pd-steps">${steps}</ol>`;
    detailsEl.classList.add("visible");
  }

  function hideMetadata(): void {
    detailsEl?.classList.remove("visible");
  }

  // ----- Persistent selection driven by clicking a path row. -----
  function selectPath(pathId: string | null): void {
    selectedPath = pathId;
    d3.select(panelEl)
      .selectAll<HTMLDivElement, GraphPath>("div.path-row")
      .classed("selected", (d) => d.id === selectedPath);

    const path = pathId ? paths.find((p) => p.id === pathId) : undefined;
    if (path) showMetadata(path);
    else hideMetadata();

    applySelection();
  }

  // Resting view: show the pinned path (if any), otherwise a neutral graph.
  function applySelection(): void {
    if (selectedPath) highlightPath(selectedPath);
    else neutralView();
  }

  function highlightPath(pathId: string): void {
    const memberNodes = new Set<string>(
      (pathNodes.get(pathId) ?? []).map((n) => n.id)
    );
    applyHighlight({
      nodes: memberNodes,
      edges: new Set(),
      paths: new Set([pathId])
    });
  }

  // ----- Shared highlight application. -----
  interface HighlightSet {
    nodes: Set<string>;
    edges: Set<string>;
    paths: Set<string>;
  }

  function applyHighlight(active: HighlightSet): void {
    nodeSel.classed("dimmed", (d) => !active.nodes.has(d.id));
    nodeSel.select("circle").classed("highlight", (d) => active.nodes.has((d as GraphNode).id));

    edgeSel
      .classed("dimmed", (d) => !active.edges.has(d.id))
      .classed("highlight", (d) => active.edges.has(d.id));

    // Only the selected paths are revealed; the rest are hidden entirely.
    setActivePaths(active.paths);
  }

  function neutralView(): void {
    nodeSel.classed("dimmed", false);
    nodeSel.select("circle").classed("highlight", false);
    edgeSel.classed("dimmed", false).classed("highlight", false);
    // Nothing selected, so no path is shown.
    setActivePaths(new Set());
  }

  // ----- Path reveal state. -----
  function setActivePaths(ids: Set<string>): void {
    activePaths.clear();
    for (const id of ids) activePaths.add(id);
    refreshPaths();
  }

  // A path is drawn only when it is selected AND all of its nodes are
  // currently visible (so collapsing a node hides paths that pass through it).
  function pathShown(pathId: string): boolean {
    if (!activePaths.has(pathId)) return false;
    const ns = pathNodes.get(pathId) ?? [];
    return ns.length > 0 && ns.every((n) => visibleNodes.has(n.id));
  }

  function refreshPaths(): void {
    pathSel.style("display", (d) => (pathShown(d.id) ? null : "none"));
    markerSel.style("display", (d) => (pathShown(d.pathId) ? null : "none"));
  }

  // ----- Expand/collapse visibility. -----
  // A node is visible if it is a root (not a child of any parent) or if it is
  // reachable through a chain of expanded, visible parents. Recomputed from
  // the per-node `expanded` flags so nested expand/collapse stays consistent.
  function computeVisible(): Set<string> {
    const visible = new Set<string>();
    for (const n of nodes) {
      if (!childIds.has(n.id)) visible.add(n.id);
    }
    let changed = true;
    while (changed) {
      changed = false;
      for (const n of nodes) {
        if (!visible.has(n.id) || !n.expanded) continue;
        for (const c of childrenOf.get(n.id) ?? []) {
          if (!visible.has(c)) {
            visible.add(c);
            changed = true;
          }
        }
      }
    }
    return visible;
  }

  // Show/hide elements for the current expand state and narrow the simulation
  // to the visible subset so hidden nodes don't participate in layout/forces.
  function applyVisibility(): void {
    const visible = computeVisible();
    visibleNodes = visible;
    const vis = (id: string): boolean => visible.has(id);
    const edgeVisible = (e: SimEdge): boolean => vis(e.source.id) && vis(e.target.id);

    nodeSel.style("display", (d) => (vis(d.id) ? null : "none"));
    nodeSel
      .select<SVGTextElement>("text.toggle")
      .text((d) => (!isExpandable(d.id) ? "" : d.expanded ? "−" : "+"));

    edgeSel.style("display", (d) => (edgeVisible(d) ? null : "none"));
    edgeLabelSel.style("display", (d) => (edgeVisible(d) ? null : "none"));
    // Re-apply the current selection so dimming/path reveal stay consistent
    // with the new node visibility.
    applySelection();

    // Rewire the simulation to the visible nodes/edges and reheat.
    simulation.nodes(nodes.filter((n) => vis(n.id)));
    linkForce.links(edges.filter((e) => edgeVisible(e)) as unknown as GraphEdge[]);
    simulation.alpha(0.6).restart();
  }
}

/**
 * Build a straight-line path string between an edge's two endpoints, trimming
 * the target end by ARROW_PADDING so the arrowhead lands just outside the
 * target node's circle. Straight (not curved) so that path overlays, which
 * connect the same node centers, sit directly on top of the structural edges.
 */
function edgePath(d: SimEdge): string {
  const sx = d.source.x ?? 0;
  const sy = d.source.y ?? 0;
  const tx = d.target.x ?? 0;
  const ty = d.target.y ?? 0;

  const dx = tx - sx;
  const dy = ty - sy;
  const dist = Math.hypot(dx, dy) || 1;

  // Trim the target end by ARROW_PADDING along the line direction.
  const ex = tx - (dx / dist) * ARROW_PADDING;
  const ey = ty - (dy / dist) * ARROW_PADDING;

  return `M${sx},${sy}L${ex},${ey}`;
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function addTo(map: Map<string, Set<string>>, key: string, value: string): void {
  let set = map.get(key);
  if (!set) {
    set = new Set();
    map.set(key, set);
  }
  set.add(value);
}

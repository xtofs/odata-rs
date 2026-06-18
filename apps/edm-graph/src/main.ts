import type { Graph } from "./types";
import { renderGraph } from "./render";

/**
 * Load the graph document from the statically-served public/graph.json.
 */
async function loadGraph(url: string): Promise<Graph> {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`Failed to load ${url}: ${res.status} ${res.statusText}`);
  }
  return (await res.json()) as Graph;
}

/**
 * Warn (but don't fail) on referential problems so a slightly malformed graph
 * still renders what it can.
 */
function validateGraph(graph: Graph): void {
  const nodeIds = new Set(graph.nodes.map((n) => n.id));
  const edgeIds = new Set(graph.edges.map((e) => e.id));

  for (const edge of graph.edges) {
    if (!nodeIds.has(edge.source)) {
      console.warn(`Edge "${edge.id}" references unknown source node "${edge.source}".`);
    }
    if (!nodeIds.has(edge.target)) {
      console.warn(`Edge "${edge.id}" references unknown target node "${edge.target}".`);
    }
  }

  for (const path of graph.paths) {
    for (const seg of path.segments) {
      if (!nodeIds.has(seg.node)) {
        console.warn(`Path "${path.id}" references unknown node "${seg.node}".`);
      }
      if (seg.edge !== null && !edgeIds.has(seg.edge)) {
        console.warn(`Path "${path.id}" references unknown edge "${seg.edge}".`);
      }
    }
  }
}

function showError(message: string): void {
  const el = document.getElementById("error");
  if (el) {
    el.textContent = message;
    el.style.display = "flex";
  }
  console.error(message);
}

async function main(): Promise<void> {
  const svg = document.getElementById("graph") as SVGSVGElement | null;
  const panel = document.getElementById("path-list") as HTMLElement | null;

  if (!svg || !panel) {
    showError("Missing #graph or #path-list elements in the page.");
    return;
  }

  try {
    const graph = await loadGraph("/graph.json");
    validateGraph(graph);
    renderGraph(graph, svg, panel);
  } catch (err) {
    showError(err instanceof Error ? err.message : String(err));
  }
}

void main();

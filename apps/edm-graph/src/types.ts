import type { SimulationNodeDatum } from "d3";

/**
 * A graph node. Extends D3's SimulationNodeDatum so the force simulation can
 * attach/mutate layout fields (x, y, vx, vy, fx, fy) directly on the object.
 */
export interface GraphNode extends SimulationNodeDatum {
  id: string;
  kind: string;
  label: string;
  /** Expand/collapse state for parent nodes (default false = collapsed). */
  expanded?: boolean;
  /** Child node ids reachable via outgoing "Declares"/"Contains" edges. */
  children?: string[];
}

/**
 * An edge as it appears in graph.json (source/target are node id strings).
 */
export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  kind: string;
  label: string;
}

/**
 * An edge after d3.forceLink() has resolved source/target string ids into the
 * actual GraphNode objects. D3 mutates the original edge objects in place, so
 * at runtime an edge is really a SimEdge once the simulation is wired up.
 */
export interface SimEdge {
  id: string;
  source: GraphNode;
  target: GraphNode;
  kind: string;
  label: string;
}

/**
 * One ordered step of a path. `node` references a node id; `edge` references an
 * edge id (or null for the first/initial segment that has no incoming edge).
 */
export interface PathSegment {
  node: string;
  edge: string | null;
  order: number;
}

/**
 * A path: an ordered sequence of segments owned by some entity.
 */
export interface GraphPath {
  id: string;
  owner: string;
  label: string;
  segments: PathSegment[];
}

/**
 * The full graph document loaded from graph.json.
 */
export interface Graph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  paths: GraphPath[];
}

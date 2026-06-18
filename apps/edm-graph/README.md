# edm-graph

A small Vite + TypeScript + D3 app that visualizes an EDM (OData/CSDL) model as a
semantic graph.

## Why

CSDL describes an OData model as nested XML/JSON. That shape hides the *graph*
underneath it: types, properties and navigation properties are nodes; the
references between them are edges; and CSDL's **path-attributes** (keys,
partners, navigation-property bindings, entity-set paths, …) are ordered walks
*through* that graph.

This app makes those structures legible — you can see how elements connect, and
trace each path-attribute as a highlighted route over the underlying edges. It is
purely a consumer: the `csdl-edm` crate is the single source of truth that
resolves CSDL and exports the graph; this app only renders it.

## Data

The app loads `public/graph.json` at runtime: a `{ nodes, edges, paths }`
document produced by the `csdl-edm` exporter. It is a drop-in file — replace it
and reload, no code changes.

Regenerate it from a CSDL model via the `csdl-edm` example (run from the
workspace root or this directory):

```bash
npm run regen                         # default sample model -> public/graph.json
npm run regen -- path/to/model.csdl.xml   # a specific model
```

`regen` shells out to `cargo run -p csdl-edm --example edm_graph_demo`, which
parses, resolves, validates, and writes `public/graph.json`.

## Run

```bash
npm install      # first time only
npm run dev      # start the dev server, open the printed URL
```

Other scripts: `npm run build` (type-check + production build), `npm run preview`
(serve the build).

## Using it

- **Nodes** are model elements; **gray edges** are structural references
  (`has` for parent/child, the CSDL attribute name otherwise).
- Click a node's **±** to expand/collapse its children; **Space** toggles
  expand-all / collapse-all.
- **Paths** are hidden until selected — click a path in the right-hand panel to
  pin it; it lights up as a colored polyline with numbered segment order, and its
  metadata appears below. Click again to deselect.
- Hover a node to preview its incident edges and paths; drag to reposition,
  scroll to zoom, drag the background to pan.

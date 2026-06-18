//! Load CSDL, resolve to the EDM semantic model, and export the semantic graph
//! as `graph.json` for the `edm-graph` visualizer.
//!
//! Usage:
//!   cargo run --example edm_graph_demo -- <input.csdl.xml|.json> [out.json]
//!
//! With no arguments it reads the bundled `fields_sample.csdl.xml` and writes
//! `graph.json` into the crate root.

use std::path::{Path, PathBuf};

use csdl_edm::CsdlDocument;
use csdl_edm::graph::build_graph;
use csdl_edm::resolver::Resolver;
use csdl_edm::validator::validate_document;

fn main() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut args = std::env::args().skip(1);
    let input: PathBuf = args.next().map_or_else(
        || {
            project_root
                .join("data")
                .join("inputs")
                .join("example89.csdl.xml")
        },
        PathBuf::from,
    );
    // Default output is the in-repo visualizer's runtime file, so a plain
    // `cargo run --example edm_graph_demo` refreshes what the app serves.
    let output: PathBuf = args.next().map_or_else(
        || project_root.join("../../apps/edm-graph/public/graph.json"),
        PathBuf::from,
    );

    let parsed = CsdlDocument::from_path(&input).expect("parse CSDL input");
    let edmx = parsed.edmx.expect("missing Edmx document");
    let edm = Resolver::resolve_edmx_document(edmx).expect("resolve to EDM");
    validate_document(&edm).expect("validate EDM");

    let graph = build_graph(&edm);
    let json = serde_json::to_string_pretty(&graph).expect("serialize graph");
    std::fs::write(&output, json).expect("write graph.json");

    println!(
        "Wrote {} nodes, {} edges, {} paths to {}",
        graph.nodes.len(),
        graph.edges.len(),
        graph.paths.len(),
        output.display()
    );
}

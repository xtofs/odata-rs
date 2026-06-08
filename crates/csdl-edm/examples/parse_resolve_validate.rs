use std::path::Path;

use csdl_edm::CsdlDocument;
use csdl_edm::resolver::Resolver;
use csdl_edm::validator::validate_document;

fn main() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = std::env::args().nth(1).map_or_else(
        || {
            project_root
                .join("data")
                .join("inputs")
                .join("fields_sample.csdl.xml")
        },
        |value| project_root.join(value),
    );

    let parsed = CsdlDocument::from_path(&input).expect("parse CSDL input");
    let edmx = parsed.edmx.expect("missing Edmx document");
    let edm = Resolver::resolve_edmx_document(edmx).expect("resolve to EDM");
    validate_document(&edm).expect("validate EDM");

    println!("Parsed, resolved, and validated {}", input.display());
}

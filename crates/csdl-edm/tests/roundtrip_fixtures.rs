use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use csdl_edm::{CsdlDocument, CsdlFormat};
use rstest::rstest;
use similar::TextDiff;

#[derive(Debug, Clone, Copy)]
enum SourceFormat {
    Json,
    Xml,
}

#[derive(Debug, Clone, Copy)]
enum TargetFormat {
    Json,
    Xml,
}

#[test]
fn roundtrip_case_list_matches_input_fixtures() {
    let mut expected = input_case_paths();
    let mut actual = input_paths();
    expected.sort();
    actual.sort();
    assert_eq!(
        actual, expected,
        "roundtrip case list is out of sync with fixture inputs"
    );
}

#[rstest]
#[case("enum_sample.csdl.json")]
#[case("enum_sample.csdl.xml")]
#[case("example89.csdl.json")]
#[case("example89.csdl.xml")]
#[case("extras_sample.csdl.json")]
#[case("extras_sample.csdl.xml")]
#[case("fields_sample.csdl.json")]
#[case("fields_sample.csdl.xml")]
#[case("import_sample.csdl.json")]
#[case("import_sample.csdl.xml")]
#[case("record_sample.csdl.json")]
#[case("record_sample.csdl.xml")]
#[case("reporting_line.csdl.json")]
#[case("reporting_line.csdl.xml")]
#[case("types_sample.csdl.json")]
#[case("types_sample.csdl.xml")]
fn fixture_roundtrips_are_idempotent_after_normalization(#[case] input_name: &str) {
    let input = input_path(input_name);
    let source = source_format(&input);
    let document = CsdlDocument::from_path(&input)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", input.display()));

    for target in [TargetFormat::Json, TargetFormat::Xml] {
        let mut out = Vec::new();
        document
            .write_as(&mut out, to_csdl_format(target))
            .unwrap_or_else(|err| {
                panic!(
                    "failed to serialize {} ({source:?}) as {:?}: {err}",
                    input.display(),
                    target
                )
            });

        let first_roundtrip = parse_document(&out, target).unwrap_or_else(|err| {
            panic!(
                "failed to parse roundtripped {} ({source:?}) as {:?}: {err}",
                input.display(),
                target
            )
        });

        let mut out_second = Vec::new();
        first_roundtrip
            .write_as(&mut out_second, to_csdl_format(target))
            .unwrap_or_else(|err| {
                panic!(
                    "failed second serialization for {} ({source:?}) as {:?}: {err}",
                    input.display(),
                    target
                )
            });

        let second_roundtrip = parse_document(&out_second, target).unwrap_or_else(|err| {
            panic!(
                "failed to parse second roundtrip {} ({source:?}) as {:?}: {err}",
                input.display(),
                target
            )
        });

        if first_roundtrip == second_roundtrip {
            continue;
        }

        let before = render_for_diff(&first_roundtrip, target);
        let after = render_for_diff(&second_roundtrip, target);
        let diff = TextDiff::from_lines(&before, &after)
            .unified_diff()
            .header("after first roundtrip", "after second roundtrip")
            .to_string();

        panic!(
            "roundtrip not idempotent for {} ({source:?} -> {:?})\n{diff}",
            input.display(),
            target
        );
    }
}

fn input_paths() -> Vec<PathBuf> {
    let mut paths = fs::read_dir(inputs_dir())
        .expect("list data/inputs")
        .map(|entry| entry.expect("valid dir entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|name| name.ends_with(".csdl.xml") || name.ends_with(".csdl.json"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn input_case_paths() -> Vec<PathBuf> {
    [
        "enum_sample.csdl.json",
        "enum_sample.csdl.xml",
        "example89.csdl.json",
        "example89.csdl.xml",
        "extras_sample.csdl.json",
        "extras_sample.csdl.xml",
        "fields_sample.csdl.json",
        "fields_sample.csdl.xml",
        "import_sample.csdl.json",
        "import_sample.csdl.xml",
        "record_sample.csdl.json",
        "record_sample.csdl.xml",
        "reporting_line.csdl.json",
        "reporting_line.csdl.xml",
        "types_sample.csdl.json",
        "types_sample.csdl.xml",
    ]
    .into_iter()
    .map(input_path)
    .collect()
}

fn input_path(file_name: &str) -> PathBuf {
    let path = inputs_dir().join(file_name);
    assert!(path.is_file(), "fixture not found: {}", path.display());
    path
}

fn to_csdl_format(target: TargetFormat) -> CsdlFormat {
    match target {
        TargetFormat::Json => CsdlFormat::Json,
        TargetFormat::Xml => CsdlFormat::Xml,
    }
}

fn source_format(path: &Path) -> SourceFormat {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<unknown>");
    if name.ends_with(".csdl.xml") {
        return SourceFormat::Xml;
    }
    if name.ends_with(".csdl.json") {
        return SourceFormat::Json;
    }

    panic!("unsupported fixture extension: {}", path.display());
}

fn parse_document(bytes: &[u8], target: TargetFormat) -> std::io::Result<CsdlDocument> {
    match target {
        TargetFormat::Json => csdl_edm::parser::from_json_reader(Cursor::new(bytes)),
        TargetFormat::Xml => csdl_edm::parser::from_xml_reader(Cursor::new(bytes)),
    }
}

fn render_for_diff(document: &CsdlDocument, target: TargetFormat) -> String {
    let mut out = Vec::new();
    document
        .write_as(&mut out, to_csdl_format(target))
        .unwrap_or_else(|err| panic!("failed to render for diff ({target:?}): {err}"));
    String::from_utf8(out).unwrap_or_else(|err| panic!("output for diff is not utf8: {err}"))
}

fn inputs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("inputs")
}

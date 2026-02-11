mod support;

use sorcat_core::{Export, ExportKind, decode_module_summary};
use support::load_wasm_fixture;

#[test]
fn decodes_sections_imports_exports_and_function_bodies() {
    let wasm = load_wasm_fixture("sections_imports_exports.wasm");

    let summary = decode_module_summary(&wasm).expect(
        "sections/imports/exports fixture should decode once sorcat-core decode is implemented",
    );

    assert_eq!(
        summary.defined_function_bodies, 2,
        "expected two defined function bodies in fixture",
    );
    assert_eq!(
        summary.import_functions.len(),
        1,
        "expected one imported function",
    );
    assert!(
        summary
            .import_functions
            .iter()
            .any(|import| import.module == "env" && import.name == "log"),
        "expected env::log import",
    );
    assert!(
        summary.exports.contains(&Export {
            name: "adder".to_owned(),
            kind: ExportKind::Function,
            index: 1,
        }),
        "expected function export adder",
    );
    assert!(
        summary.exports.contains(&Export {
            name: "call_log".to_owned(),
            kind: ExportKind::Function,
            index: 2,
        }),
        "expected function export call_log",
    );
    assert!(
        summary.exports.contains(&Export {
            name: "memory".to_owned(),
            kind: ExportKind::Memory,
            index: 0,
        }),
        "expected memory export",
    );
}

#[test]
fn decodes_expected_opcode_sequence_for_simple_arithmetic_function() {
    let wasm = load_wasm_fixture("sections_imports_exports.wasm");

    let summary = decode_module_summary(&wasm)
        .expect("function body decoding should succeed once sorcat-core decode is implemented");
    let adder_body = summary
        .function_bodies
        .iter()
        .find(|body| body.symbol == "adder")
        .expect("expected body summary for adder export");

    assert_eq!(
        adder_body.opcodes,
        vec!["local.get", "local.get", "i32.add", "end"],
        "expected deterministic opcode rendering for adder",
    );
}

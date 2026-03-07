use std::path::PathBuf;

use sorcat_core::{
    DecodedModuleSummary, Export, ExportKind, FunctionBodySummary, ImportFunction, Instruction,
};
use sorcat_rust_backend::{
    RustBackendError, reconstruct_module, reconstruct_module_from_wasm,
};

fn load_wasm_fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/wasm")
        .join(name);
    std::fs::read(&path)
        .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()))
}

#[test]
fn reconstruct_from_wasm_is_deterministic() {
    let wasm = load_wasm_fixture("sections_imports_exports.wasm");
    let first = reconstruct_module_from_wasm(&wasm).expect("should succeed");
    let second = reconstruct_module_from_wasm(&wasm).expect("should succeed");
    assert_eq!(first, second, "reconstruction must be deterministic");
}

#[test]
fn reconstruct_from_wasm_contains_expected_structure() {
    let wasm = load_wasm_fixture("sections_imports_exports.wasm");
    let output = reconstruct_module_from_wasm(&wasm).expect("should succeed");
    assert!(
        output.contains("pub mod decompiled"),
        "expected decompiled module wrapper"
    );
    assert!(
        output.contains("// defined function bodies:"),
        "expected function body count comment"
    );
}

#[test]
fn reconstruct_from_wasm_soroban_fixture_has_annotations() {
    let wasm = load_wasm_fixture("soroban_env_imports.wasm");
    let output = reconstruct_module_from_wasm(&wasm).expect("should succeed");
    assert!(
        output.contains("soroban_builtin=") || output.contains("host_"),
        "expected soroban builtin annotations in reconstruction"
    );
}

#[test]
fn reconstruct_module_rejects_count_mismatch() {
    let summary = DecodedModuleSummary {
        import_functions: vec![],
        exports: vec![],
        defined_function_bodies: 1,
        function_bodies: vec![],
    };
    let error = reconstruct_module(&summary).expect_err("should reject mismatch");
    assert!(
        matches!(error, RustBackendError::InvalidInput { .. }),
        "expected InvalidInput error"
    );
}

#[test]
fn reconstruct_module_handles_empty_bodies() {
    let summary = DecodedModuleSummary {
        import_functions: vec![],
        exports: vec![],
        defined_function_bodies: 0,
        function_bodies: vec![],
    };
    let output = reconstruct_module(&summary).expect("should succeed for empty module");
    assert!(output.contains("pub mod decompiled"));
    assert!(output.contains("// reconstructed functions: 0"));
}

#[test]
fn reconstruct_module_disambiguates_colliding_symbols() {
    let summary = DecodedModuleSummary {
        import_functions: vec![
            ImportFunction {
                module: "env".to_string(),
                name: "host-fn".to_string(),
                params: vec![],
                results: vec![],
            },
            ImportFunction {
                module: "env".to_string(),
                name: "host_fn".to_string(),
                params: vec![],
                results: vec![],
            },
        ],
        exports: vec![],
        defined_function_bodies: 2,
        function_bodies: vec![
            FunctionBodySummary {
                function_index: 1,
                symbol: "fn-a".to_string(),
                params: vec![],
                results: vec![],
                opcodes: vec!["end".to_string()],
                instructions: vec![],
            },
            FunctionBodySummary {
                function_index: 2,
                symbol: "fn_a".to_string(),
                params: vec![],
                results: vec![],
                opcodes: vec!["end".to_string()],
                instructions: vec![],
            },
        ],
    };
    let output = reconstruct_module(&summary).expect("should succeed");
    // Both fn-a and fn_a sanitize to fn_a, so the second must get a suffix
    assert!(
        output.contains("fn fn_a(") || output.contains("fn fn_a()"),
        "first function should keep base symbol"
    );
    assert!(
        output.contains("fn fn_a_1(") || output.contains("fn fn_a_1()"),
        "second function should get deterministic suffix"
    );
}

#[test]
fn reconstruct_from_wasm_rejects_malformed_binary() {
    let bad_wasm = vec![0x00, 0x00, 0x00, 0x00]; // bad magic
    let error = reconstruct_module_from_wasm(&bad_wasm).expect_err("should reject bad magic");
    assert!(
        matches!(error, RustBackendError::Core(_)),
        "expected Core error for malformed input"
    );
}

#[test]
fn reconstruct_module_with_instructions_produces_rust_body() {
    let summary = DecodedModuleSummary {
        import_functions: vec![],
        exports: vec![Export {
            name: "add".to_string(),
            kind: ExportKind::Function,
            index: 0,
        }],
        defined_function_bodies: 1,
        function_bodies: vec![FunctionBodySummary {
            function_index: 0,
            symbol: "add".to_string(),
            params: vec![],
            results: vec![],
            opcodes: vec![
                "local.get".to_string(),
                "local.get".to_string(),
                "i32.add".to_string(),
                "end".to_string(),
            ],
            instructions: vec![
                Instruction::LocalGet { local_index: 0 },
                Instruction::LocalGet { local_index: 1 },
                Instruction::I32Add,
                Instruction::End,
            ],
        }],
    };
    let output = reconstruct_module(&summary).expect("should succeed");
    assert!(
        output.contains("fn add"),
        "expected 'add' function in output"
    );
}

mod support;

use sorcat_core::{decode_module_summary, lift_function_to_ssa_summary, CoreErrorKind};
use support::load_wasm_fixture;

#[test]
fn rejects_malformed_binary_with_explicit_error_kind() {
    let wasm = load_wasm_fixture("malformed_bad_magic.wasm");

    let error = decode_module_summary(&wasm)
        .expect_err("malformed fixture must return a structured error once decode pipeline exists");

    assert_eq!(
        error.kind,
        CoreErrorKind::MalformedBinary,
        "malformed fixture must map to MalformedBinary",
    );
    assert!(
        error.message.contains("magic") || error.message.contains("malformed"),
        "malformed fixture error should explain decode failure source",
    );
}

#[test]
fn rejects_truncated_code_section_payload_as_malformed() {
    let malformed = vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section: one () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function with type 0
        0x0a, 0x05, 0x01, 0x02, 0x00, 0x0b, // truncated code section payload
    ];

    let error = decode_module_summary(&malformed)
        .expect_err("truncated section payloads must be rejected as malformed");

    assert_eq!(error.kind, CoreErrorKind::MalformedBinary);
    assert!(
        error
            .message
            .contains("section payload extends past end of binary"),
        "error should identify truncated section payloads",
    );
}

#[test]
fn rejects_unsupported_call_indirect_in_lifting_stage() {
    let wasm = load_wasm_fixture("unsupported_call_indirect.wasm");

    let error = lift_function_to_ssa_summary(&wasm, "dispatch").expect_err(
        "unsupported fixture must return UnsupportedConstruct once SSA lifting is implemented",
    );

    assert_eq!(
        error.kind,
        CoreErrorKind::UnsupportedConstruct,
        "unsupported construct fixture must map to UnsupportedConstruct",
    );
    assert!(
        error.message.contains("call_indirect"),
        "unsupported construct error should name call_indirect",
    );
}

#[test]
fn rejects_function_bodies_with_trailing_opcodes_after_end() {
    let malformed = vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section: one () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function with type 0
        0x0a, 0x05, 0x01, 0x03, 0x00, 0x0b,
        0x0b, // code section: body has extra opcode after end
    ];

    let error = decode_module_summary(&malformed)
        .expect_err("function bodies with opcodes after terminal end must be rejected");

    assert_eq!(
        error.kind,
        CoreErrorKind::MalformedBinary,
        "malformed body should map to MalformedBinary",
    );
    assert!(
        error.message.contains("trailing opcodes"),
        "error should explain why function body is malformed",
    );
}

#[test]
fn rejects_else_without_active_if_block() {
    let malformed = vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section: one () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function with type 0
        0x0a, 0x05, 0x01, 0x03, 0x00, 0x05, 0x0b, // code section: else without if
    ];

    let error = decode_module_summary(&malformed)
        .expect_err("else without active if block must be rejected");

    assert_eq!(error.kind, CoreErrorKind::MalformedBinary);
    assert!(
        error
            .message
            .contains("else opcode without active if block"),
        "error should explain malformed else structure",
    );
}

#[test]
fn rejects_non_mvp_block_type_index_in_function_body() {
    let malformed = vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section: one () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function with type 0
        0x0a, 0x07, 0x01, 0x05, 0x00, 0x02, 0x00, 0x0b,
        0x0b, // block with non-MVP block type (type index)
    ];

    let error =
        decode_module_summary(&malformed).expect_err("non-MVP block type indices must be rejected");

    assert_eq!(
        error.kind,
        CoreErrorKind::UnsupportedConstruct,
        "non-MVP block type should map to UnsupportedConstruct",
    );
    assert!(
        error.message.contains("non-MVP block type"),
        "error should explain unsupported block type",
    );
}

use sorcat_core::{
    CoreErrorKind, ParseLimits, decode_module_summary_with_limits,
    lift_function_to_ssa_summary_with_limits,
};

#[test]
fn rejects_wasm_larger_than_configured_byte_limit() {
    let wasm = wasm_with_empty_exported_function();

    let limits = ParseLimits {
        max_wasm_bytes: 8,
        ..ParseLimits::default()
    };

    let error = decode_module_summary_with_limits(&wasm, &limits)
        .expect_err("oversized wasm should be rejected under max_wasm_bytes");

    assert_eq!(error.kind, CoreErrorKind::ResourceLimitExceeded);
    assert!(
        error.message.contains("max_wasm_bytes"),
        "error should point to max_wasm_bytes guard"
    );
}

#[test]
fn rejects_function_exceeding_instruction_limit() {
    let wasm = wasm_with_many_instructions();

    let limits = ParseLimits {
        max_instructions_per_function: 4,
        ..ParseLimits::default()
    };

    let error = decode_module_summary_with_limits(&wasm, &limits)
        .expect_err("functions with too many instructions should be rejected");

    assert_eq!(error.kind, CoreErrorKind::ResourceLimitExceeded);
    assert!(
        error.message.contains("max_instructions_per_function"),
        "error should point to instruction-count guard"
    );
}

#[test]
fn rejects_function_exceeding_block_nesting_limit() {
    let wasm = wasm_with_nested_blocks();

    let limits = ParseLimits {
        max_block_nesting_depth: 2,
        ..ParseLimits::default()
    };

    let error = decode_module_summary_with_limits(&wasm, &limits)
        .expect_err("nesting above the configured block depth should be rejected");

    assert_eq!(error.kind, CoreErrorKind::ResourceLimitExceeded);
    assert!(
        error.message.contains("max_block_nesting_depth"),
        "error should point to block depth guard"
    );
}

#[test]
fn lift_path_enforces_the_same_limits() {
    let wasm = wasm_with_many_instructions();

    let limits = ParseLimits {
        max_instructions_per_function: 4,
        ..ParseLimits::default()
    };

    let error = lift_function_to_ssa_summary_with_limits(&wasm, "f", &limits)
        .expect_err("SSA lift path must enforce parser limits consistently");

    assert_eq!(error.kind, CoreErrorKind::ResourceLimitExceeded);
}

fn wasm_with_empty_exported_function() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type: () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function type 0
        0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, // export "f"
        0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b, // code: empty function body
    ]
}

fn wasm_with_many_instructions() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // type: () -> i32
        0x03, 0x02, 0x01, 0x00, // function section: one function type 0
        0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, // export "f"
        0x0a, 0x0c, 0x01, 0x0a, 0x00, // code section and body header
        0x41, 0x01, // i32.const 1
        0x1a, // drop
        0x41, 0x02, // i32.const 2
        0x1a, // drop
        0x41, 0x03, // i32.const 3
        0x0b, // end
    ]
}

fn wasm_with_nested_blocks() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type: () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function type 0
        0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, // export "f"
        0x0a, 0x0d, 0x01, 0x0b, 0x00, // code section and body header
        0x02, 0x40, // block
        0x02, 0x40, // block
        0x02, 0x40, // block
        0x0b, // end block
        0x0b, // end block
        0x0b, // end block
        0x0b, // end function
    ]
}

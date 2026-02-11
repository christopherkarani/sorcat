use sorcat_core::{decode_module_summary, CoreErrorKind, Instruction};

#[test]
fn decoder_retains_instruction_immediates_in_deterministic_order() {
    let wasm = vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x05, 0x01, 0x60, 0x01, 0x7f, 0x00, // type: (i32) -> ()
        0x03, 0x02, 0x01, 0x00, // function: one function, type 0
        0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, // export: "f" func 0
        0x0a, 0x13, 0x01, // code: one function body, 19 bytes payload
        0x11, // body size: 17 bytes
        0x00, // local decl count
        0x02, 0x40, // block
        0x20, 0x00, // local.get 0
        0x21, 0x00, // local.set 0
        0x41, 0x07, // i32.const 7
        0x0d, 0x00, // br_if 0
        0x10, 0x00, // call 0
        0x0c, 0x00, // br 0
        0x0b, // end (block)
        0x0b, // end (function)
    ];

    let summary =
        decode_module_summary(&wasm).expect("well-formed wasm should decode successfully");
    assert_eq!(
        summary.defined_function_bodies, 1,
        "expected one defined body",
    );
    assert_eq!(summary.function_bodies.len(), 1);

    let body = &summary.function_bodies[0];
    assert_eq!(body.symbol, "f");
    assert_eq!(
        body.opcodes,
        vec![
            "block",
            "local.get",
            "local.set",
            "i32.const",
            "br_if",
            "call",
            "br",
            "end",
            "end"
        ],
        "expected opcode names to remain stable",
    );
    assert_eq!(
        body.instructions,
        vec![
            Instruction::Block,
            Instruction::LocalGet { local_index: 0 },
            Instruction::LocalSet { local_index: 0 },
            Instruction::I32Const { value: 7 },
            Instruction::BrIf { depth: 0 },
            Instruction::Call { function_index: 0 },
            Instruction::Br { depth: 0 },
            Instruction::End,
            Instruction::End,
        ],
        "expected instruction immediates to be preserved",
    );
}

#[test]
fn rejects_truncated_i64_const_immediate_as_malformed() {
    let wasm = vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type: () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function with type 0
        0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, // export "f"
        0x0a, 0x05, 0x01, 0x03, 0x00, 0x42, 0x80, // truncated i64.const immediate payload
    ];

    let error = decode_module_summary(&wasm).expect_err("truncated immediates must be rejected");

    assert_eq!(error.kind, CoreErrorKind::MalformedBinary);
    assert!(
        error.message.contains("i64.const literal"),
        "error should name the malformed immediate context"
    );
}

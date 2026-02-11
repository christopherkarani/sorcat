use sorcat_core::{CoreErrorKind, Instruction, decode_module_summary};

#[test]
fn decodes_extended_soroban_relevant_opcode_subset() {
    let wasm = wasm_with_i64_globals_and_compares();

    let summary = decode_module_summary(&wasm)
        .expect("extended opcode coverage fixture should decode successfully");

    let math = summary
        .function_bodies
        .iter()
        .find(|body| body.symbol == "math64")
        .expect("expected exported function math64 in summary");

    assert_eq!(
        math.opcodes,
        vec![
            "local.get",
            "local.tee",
            "global.set",
            "global.get",
            "i64.const",
            "i64.mul",
            "i64.const",
            "i64.sub",
            "local.get",
            "i64.add",
            "end"
        ],
        "expected deterministic opcode rendering for extended i64/global/local.tee coverage",
    );

    assert_eq!(
        math.instructions,
        vec![
            Instruction::LocalGet { local_index: 0 },
            Instruction::LocalTee { local_index: 0 },
            Instruction::GlobalSet { global_index: 0 },
            Instruction::GlobalGet { global_index: 0 },
            Instruction::I64Const { value: 2 },
            Instruction::I64Mul,
            Instruction::I64Const { value: 1 },
            Instruction::I64Sub,
            Instruction::LocalGet { local_index: 0 },
            Instruction::I64Add,
            Instruction::End,
        ],
    );

    let cmp = summary
        .function_bodies
        .iter()
        .find(|body| body.symbol == "cmp64")
        .expect("expected exported function cmp64 in summary");

    assert!(
        cmp.instructions.contains(&Instruction::I64GtS),
        "expected i64.gt_s compare to be preserved in typed IR"
    );
    assert!(
        cmp.instructions.contains(&Instruction::If)
            && cmp.instructions.contains(&Instruction::Else),
        "expected structured branch opcodes to be preserved"
    );
}

#[test]
fn rejects_unknown_opcode_with_structured_unsupported_error() {
    let wasm = vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section: one () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function type 0
        0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, // export "f"
        0x0a, 0x05, 0x01, 0x03, 0x00, 0xfc, 0x0b, // code with unsupported opcode 0xfc
    ];

    let error = decode_module_summary(&wasm)
        .expect_err("unknown opcodes must not be ignored or silently dropped");

    assert_eq!(error.kind, CoreErrorKind::UnsupportedConstruct);
    assert!(
        error.message.contains("opcode 0xfc"),
        "error should include unsupported opcode byte"
    );
}

fn wasm_with_i64_globals_and_compares() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        // type section
        0x01, 0x0b, 0x02,
        0x60, 0x01, 0x7e, 0x01, 0x7e, // (i64) -> i64
        0x60, 0x01, 0x7e, 0x01, 0x7f, // (i64) -> i32
        // function section
        0x03, 0x03, 0x02, 0x00, 0x01,
        // global section: (mut i64) initialized to 7
        0x06, 0x06, 0x01, 0x7e, 0x01, 0x42, 0x07, 0x0b,
        // export section
        0x07, 0x12, 0x02,
        0x06, b'm', b'a', b't', b'h', b'6', b'4', 0x00, 0x00,
        0x05, b'c', b'm', b'p', b'6', b'4', 0x00, 0x01,
        // code section
        0x0a, 0x25, 0x02,
        // function 0 (math64)
        0x13,
        0x00,
        0x20, 0x00,
        0x22, 0x00,
        0x24, 0x00,
        0x23, 0x00,
        0x42, 0x02,
        0x7e,
        0x42, 0x01,
        0x7d,
        0x20, 0x00,
        0x7c,
        0x0b,
        // function 1 (cmp64)
        0x0f,
        0x00,
        0x20, 0x00,
        0x42, 0x00,
        0x55,
        0x04, 0x7f,
        0x41, 0x01,
        0x05,
        0x41, 0x00,
        0x0b,
        0x0b,
    ]
}

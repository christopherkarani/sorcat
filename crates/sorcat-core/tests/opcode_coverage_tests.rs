use sorcat_core::{decode_module_summary, CoreErrorKind, Instruction};

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

#[test]
fn decodes_common_integer_opcode_expansions_in_stable_order() {
    let wasm = wasm_with_single_exported_function(&[
        0x49, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f, // i32 unsigned and boundary compares
        0x54, 0x56, 0x57, 0x58, 0x59, 0x5a, // i64 unsigned and boundary compares
        0x6d, 0x6e, 0x6f, 0x70, // i32 div/rem
        0x71, 0x72, 0x73, 0x74, 0x75, 0x76, // i32 bitwise/shift
        0x7f, 0x80, 0x81, 0x82, // i64 div/rem
        0x83, 0x84, 0x85, 0x86, 0x87, 0x88, // i64 bitwise/shift
        0x0b, // end
    ]);

    let summary = decode_module_summary(&wasm)
        .expect("extended integer opcode subset should decode successfully");
    let body = summary
        .function_bodies
        .iter()
        .find(|candidate| candidate.symbol == "f")
        .expect("expected exported function f in summary");

    assert_eq!(
        body.opcodes,
        vec![
            "i32.lt_u",
            "i32.gt_u",
            "i32.le_s",
            "i32.le_u",
            "i32.ge_s",
            "i32.ge_u",
            "i64.lt_u",
            "i64.gt_u",
            "i64.le_s",
            "i64.le_u",
            "i64.ge_s",
            "i64.ge_u",
            "i32.div_s",
            "i32.div_u",
            "i32.rem_s",
            "i32.rem_u",
            "i32.and",
            "i32.or",
            "i32.xor",
            "i32.shl",
            "i32.shr_s",
            "i32.shr_u",
            "i64.div_s",
            "i64.div_u",
            "i64.rem_s",
            "i64.rem_u",
            "i64.and",
            "i64.or",
            "i64.xor",
            "i64.shl",
            "i64.shr_s",
            "i64.shr_u",
            "end",
        ],
        "new opcode names and order must remain deterministic",
    );

    assert_eq!(
        body.instructions,
        vec![
            Instruction::I32LtU,
            Instruction::I32GtU,
            Instruction::I32LeS,
            Instruction::I32LeU,
            Instruction::I32GeS,
            Instruction::I32GeU,
            Instruction::I64LtU,
            Instruction::I64GtU,
            Instruction::I64LeS,
            Instruction::I64LeU,
            Instruction::I64GeS,
            Instruction::I64GeU,
            Instruction::I32DivS,
            Instruction::I32DivU,
            Instruction::I32RemS,
            Instruction::I32RemU,
            Instruction::I32And,
            Instruction::I32Or,
            Instruction::I32Xor,
            Instruction::I32Shl,
            Instruction::I32ShrS,
            Instruction::I32ShrU,
            Instruction::I64DivS,
            Instruction::I64DivU,
            Instruction::I64RemS,
            Instruction::I64RemU,
            Instruction::I64And,
            Instruction::I64Or,
            Instruction::I64Xor,
            Instruction::I64Shl,
            Instruction::I64ShrS,
            Instruction::I64ShrU,
            Instruction::End,
        ],
        "typed IR must preserve the expanded opcode subset",
    );
}

fn wasm_with_i64_globals_and_compares() -> Vec<u8> {
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        // type section
        0x01, 0x0b, 0x02, 0x60, 0x01, 0x7e, 0x01, 0x7e, // (i64) -> i64
        0x60, 0x01, 0x7e, 0x01, 0x7f, // (i64) -> i32
        // function section
        0x03, 0x03, 0x02, 0x00, 0x01, // global section: (mut i64) initialized to 7
        0x06, 0x06, 0x01, 0x7e, 0x01, 0x42, 0x07, 0x0b, // export section
        0x07, 0x12, 0x02, 0x06, b'm', b'a', b't', b'h', b'6', b'4', 0x00, 0x00, 0x05, b'c', b'm',
        b'p', b'6', b'4', 0x00, 0x01, // code section
        0x0a, 0x25, 0x02, // function 0 (math64)
        0x13, 0x00, 0x20, 0x00, 0x22, 0x00, 0x24, 0x00, 0x23, 0x00, 0x42, 0x02, 0x7e, 0x42, 0x01,
        0x7d, 0x20, 0x00, 0x7c, 0x0b, // function 1 (cmp64)
        0x0f, 0x00, 0x20, 0x00, 0x42, 0x00, 0x55, 0x04, 0x7f, 0x41, 0x01, 0x05, 0x41, 0x00, 0x0b,
        0x0b,
    ]
}

fn wasm_with_single_exported_function(body_ops: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(body_ops.len() + 1);
    body.push(0x00); // local declaration count
    body.extend_from_slice(body_ops);

    let mut code_payload = vec![0x01]; // one function body
    append_u32_leb(
        u32::try_from(body.len()).expect("function body length must fit in u32"),
        &mut code_payload,
    );
    code_payload.extend_from_slice(&body);

    let mut wasm = vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section: one () -> ()
        0x03, 0x02, 0x01, 0x00, // function section: one function type 0
        0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, // export "f"
        0x0a, // code section id
    ];
    append_u32_leb(
        u32::try_from(code_payload.len()).expect("code payload length must fit in u32"),
        &mut wasm,
    );
    wasm.extend_from_slice(&code_payload);
    wasm
}

fn append_u32_leb(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let low = value & 0x7f;
        value >>= 7;
        if value == 0 {
            out.push(low as u8);
            return;
        }
        out.push((low as u8) | 0x80);
    }
}

use sorcat_core::{
    CoreErrorKind, SorobanSpecTypeKind, decode_soroban_custom_sections,
};

fn push_leb_u32(mut value: u32, output: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn encode_name(name: &str) -> Vec<u8> {
    let bytes = name.as_bytes();
    let mut out = Vec::new();
    push_leb_u32(bytes.len() as u32, &mut out);
    out.extend_from_slice(bytes);
    out
}

fn wasm_custom_section(name: &str, data: &[u8]) -> Vec<u8> {
    let mut payload = encode_name(name);
    payload.extend_from_slice(data);

    let mut section = Vec::new();
    section.push(0x00); // custom section id
    push_leb_u32(payload.len() as u32, &mut section);
    section.extend_from_slice(&payload);
    section
}

fn wasm_module_with_custom_sections(sections: &[(&str, &[u8])]) -> Vec<u8> {
    let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    for (name, data) in sections {
        wasm.extend_from_slice(&wasm_custom_section(name, data));
    }
    wasm
}

#[test]
fn decodes_soroban_custom_sections_into_typed_semantics() {
    let spec = br#"
fn|transfer|from:Address,to:Address,amount:i128|Result<(), ContractError>|Move funds
type|struct|Allowance|spender:Address,amount:i128
type|enum|Role|Admin,User
error|InsufficientBalance|1|Not enough funds
"#;

    let meta = br#"
contract_name=token
version=1.2.3
entry|authors|stellar
entry|description|token contract
"#;

    let env_meta = br#"
protocol=25
interface_version=1
sdk_version=25.0.1
"#;

    let wasm = wasm_module_with_custom_sections(&[
        ("contractspecv0", spec),
        ("contractmetav0", meta),
        ("contractenvmetav0", env_meta),
    ]);

    let decoded = decode_soroban_custom_sections(&wasm)
        .expect("soroban custom sections should decode with structured typed output");

    let spec = decoded
        .contract_spec
        .expect("contractspecv0 should be decoded");
    assert_eq!(spec.functions.len(), 1);
    assert_eq!(spec.functions[0].name, "transfer");
    assert_eq!(spec.functions[0].inputs.len(), 3);
    assert_eq!(spec.functions[0].output.as_deref(), Some("Result<(), ContractError>"));

    assert_eq!(spec.types.len(), 2);
    assert_eq!(spec.types[0].name, "Allowance");
    assert_eq!(spec.types[0].kind, SorobanSpecTypeKind::Struct);
    assert_eq!(spec.types[1].kind, SorobanSpecTypeKind::Enum);

    assert_eq!(spec.errors.len(), 1);
    assert_eq!(spec.errors[0].name, "InsufficientBalance");
    assert_eq!(spec.errors[0].code, 1);

    let meta = decoded
        .contract_meta
        .expect("contractmetav0 should be decoded");
    assert_eq!(meta.contract_name.as_deref(), Some("token"));
    assert_eq!(meta.version.as_deref(), Some("1.2.3"));
    assert_eq!(meta.entries.len(), 2);

    let env_meta = decoded
        .contract_env_meta
        .expect("contractenvmetav0 should be decoded");
    assert_eq!(env_meta.protocol, 25);
    assert_eq!(env_meta.interface_version, Some(1));
    assert_eq!(env_meta.sdk_version.as_deref(), Some("25.0.1"));
}

#[test]
fn rejects_malformed_soroban_custom_section_payload_with_structured_error() {
    let malformed_spec = br#"fn|broken|missing_type_field"#;

    let wasm = wasm_module_with_custom_sections(&[("contractspecv0", malformed_spec)]);

    let error = decode_soroban_custom_sections(&wasm)
        .expect_err("malformed section payloads must return structured errors");

    assert_eq!(error.kind, CoreErrorKind::MalformedBinary);
    assert!(
        error.message.contains("contractspecv0") && error.message.contains("decode"),
        "error should identify the malformed section and decode path"
    );
}

use std::collections::BTreeMap;

use sorcat_core::extract_custom_sections_by_name;

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
fn extracts_custom_sections_grouped_by_name() {
    let wasm = wasm_module_with_custom_sections(&[
        ("contractspecv0", &[0x01, 0x02, 0x03]),
        ("contractmetav0", &[0xaa]),
        ("contractspecv0", &[0x04]),
    ]);

    let sections = extract_custom_sections_by_name(&wasm)
        .expect("custom section extraction should succeed for valid wasm header");

    let mut expected = BTreeMap::new();
    expected.insert(
        "contractspecv0".to_string(),
        vec![vec![0x01, 0x02, 0x03], vec![0x04]],
    );
    expected.insert("contractmetav0".to_string(), vec![vec![0xaa]]);

    assert_eq!(sections, expected);
}

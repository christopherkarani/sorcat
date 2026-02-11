mod common;

use std::collections::BTreeSet;
use std::fs;

use serde_json::Value;
use sorcat_core::{Instruction, decode_module_summary};
use sorcat_eval::{
    EvalErrorKind,
    corpus::{
        BuildProfile, CorpusCategory, collect_real_world_provenance_status, load_manifest,
        validate_corpus_layout,
    },
};

const MIN_TOTAL_CONTRACTS: usize = 40;
const MIN_REAL_WORLD_CONTRACTS: usize = 20;
const MIN_SYNTHETIC_CONTRACTS: usize = 10;
const MIN_ADVERSARIAL_CONTRACTS: usize = 10;
const MIN_SDK_VERSIONS: usize = 2;

#[test]
fn manifest_loader_enforces_plan_level_corpus_minima_and_matrix_coverage() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    assert_eq!(
        manifest.schema_version, "1.0.0",
        "manifest schema version must be explicit and deterministic"
    );
    assert!(
        manifest.locked,
        "committed corpus fixture must remain locked"
    );

    assert!(
        manifest.contracts.len() >= MIN_TOTAL_CONTRACTS,
        "locked corpus must include at least {MIN_TOTAL_CONTRACTS} contracts"
    );

    let mut real_world_contracts = 0usize;
    let mut synthetic_contracts = 0usize;
    let mut adversarial_contracts = 0usize;
    let mut observed_profiles = BTreeSet::new();
    let mut observed_debug_name_modes = BTreeSet::new();
    let mut observed_sdk_versions = BTreeSet::new();

    for contract in &manifest.contracts {
        match &contract.category {
            CorpusCategory::RealWorld => real_world_contracts += 1,
            CorpusCategory::Synthetic => synthetic_contracts += 1,
            CorpusCategory::Adversarial => adversarial_contracts += 1,
        }

        for variant in &contract.variants {
            let profile = match &variant.profile {
                BuildProfile::Debug => "debug",
                BuildProfile::Release => "release",
            };
            observed_profiles.insert(profile.to_string());
            observed_debug_name_modes.insert(variant.include_debug_names);
            observed_sdk_versions.insert(variant.sdk_version.clone());
        }
    }

    assert!(
        real_world_contracts >= MIN_REAL_WORLD_CONTRACTS,
        "locked corpus must include at least {MIN_REAL_WORLD_CONTRACTS} real_world contracts"
    );
    assert!(
        synthetic_contracts >= MIN_SYNTHETIC_CONTRACTS,
        "locked corpus must include at least {MIN_SYNTHETIC_CONTRACTS} synthetic contracts"
    );
    assert!(
        adversarial_contracts >= MIN_ADVERSARIAL_CONTRACTS,
        "locked corpus must include at least {MIN_ADVERSARIAL_CONTRACTS} adversarial contracts"
    );

    let expected_profiles = ["debug".to_string(), "release".to_string()]
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed_profiles, expected_profiles,
        "locked corpus must cover both debug and release profiles"
    );

    let expected_debug_name_modes = [false, true].into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        observed_debug_name_modes, expected_debug_name_modes,
        "locked corpus must cover with/without debug names variants"
    );

    assert!(
        observed_sdk_versions.len() >= MIN_SDK_VERSIONS,
        "locked corpus must include at least {MIN_SDK_VERSIONS} sdk versions"
    );
}

#[test]
fn committed_corpus_layout_passes_manifest_validation() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    validate_corpus_layout(common::corpus_root(), &manifest)
        .expect("committed fixtures/corpus layout must satisfy manifest validation");
}

#[test]
fn committed_wasm_variants_have_per_contract_uniqueness_signal() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    let mut debug_payloads = BTreeSet::new();
    let mut release_payloads = BTreeSet::new();

    for contract in &manifest.contracts {
        for variant in &contract.variants {
            let wasm_path = common::corpus_root().join(&variant.wasm_path);
            let bytes = fs::read(&wasm_path).unwrap_or_else(|err| {
                panic!(
                    "failed to read committed wasm fixture {}: {err}",
                    wasm_path.display()
                )
            });

            match variant.profile {
                BuildProfile::Debug => {
                    debug_payloads.insert(semantic_fingerprint(&bytes));
                }
                BuildProfile::Release => {
                    release_payloads.insert(semantic_fingerprint(&bytes));
                }
            }
        }
    }

    assert!(
        debug_payloads.len() >= MIN_TOTAL_CONTRACTS,
        "debug variants should carry at least one unique semantic fingerprint per contract",
    );
    assert!(
        release_payloads.len() >= MIN_TOTAL_CONTRACTS,
        "release variants should carry at least one unique semantic fingerprint per contract",
    );
}

#[test]
fn committed_corpus_sources_must_not_be_generated_decompiler_summaries() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    const MARKER: &str = "// sorcat deterministic pseudo-rust summary v0";
    const OTHER_MARKERS: [&str; 4] = [
        "pub mod decompiled {",
        "defined function bodies:",
        "reconstructed functions:",
        "opcodes:",
    ];

    for contract in &manifest.contracts {
        let path = common::corpus_root().join(&contract.rust_source);
        let source = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!(
                "failed to read committed corpus rust source {}: {err}",
                path.display()
            )
        });

        assert!(
            !source.starts_with(MARKER),
            "corpus rust sources must be authored, not decompiler output: {}",
            path.display()
        );

        for marker in OTHER_MARKERS {
            assert!(
                !source.contains(marker),
                "corpus rust sources must not contain decompiler summary marker `{marker}`: {}",
                path.display()
            );
        }
    }
}

#[test]
fn committed_real_world_metadata_must_include_source_provenance() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    const REQUIRED_PROVENANCE_FIELDS: [&str; 5] = [
        "upstream_repo_url",
        "upstream_commit",
        "upstream_license",
        "source_origin",
        "build_recipe",
    ];

    for contract in &manifest.contracts {
        if !matches!(contract.category, CorpusCategory::RealWorld) {
            continue;
        }

        let metadata_path = common::corpus_root().join(&contract.metadata_path);
        let metadata_text = fs::read_to_string(&metadata_path).unwrap_or_else(|err| {
            panic!(
                "failed to read committed corpus metadata {}: {err}",
                metadata_path.display()
            )
        });
        let metadata_json: Value = serde_json::from_str(&metadata_text).unwrap_or_else(|err| {
            panic!(
                "failed to parse committed corpus metadata {}: {err}",
                metadata_path.display()
            )
        });
        let metadata_obj = metadata_json.as_object().unwrap_or_else(|| {
            panic!(
                "metadata must be a JSON object for {}",
                metadata_path.display()
            )
        });

        let provenance_obj = metadata_obj
            .get("source_provenance")
            .and_then(Value::as_object)
            .unwrap_or_else(|| {
                panic!(
                    "real_world metadata must include `source_provenance` object: {}",
                    metadata_path.display()
                )
            });

        for field in REQUIRED_PROVENANCE_FIELDS {
            let value = provenance_obj
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or("");
            assert!(
                !value.trim().is_empty(),
                "real_world metadata provenance field `{field}` must be non-empty: {}",
                metadata_path.display()
            );
            let lower = value.to_ascii_lowercase();
            assert!(
                !lower.contains("example.invalid")
                    && !lower.contains("example.com")
                    && !lower.contains("placeholder")
                    && !lower.contains("todo")
                    && !lower.contains("tbd")
                    && !lower.contains("locked-corpus-v1-seq")
                    && !lower.contains("curated_fixture_seed"),
                "real_world metadata provenance field `{field}` must not contain placeholder-like values: {}",
                metadata_path.display()
            );
        }

        let upstream_repo_url = provenance_obj
            .get("upstream_repo_url")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(
            upstream_repo_url.starts_with("https://"),
            "real_world metadata must use https upstream_repo_url: {}",
            metadata_path.display()
        );

        let upstream_commit = provenance_obj
            .get("upstream_commit")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        assert!(
            upstream_commit.len() == 40 && upstream_commit.chars().all(|ch| ch.is_ascii_hexdigit()),
            "real_world metadata upstream_commit must be a full 40-character commit hash: {}",
            metadata_path.display()
        );

        let verification_status = provenance_obj
            .get("verification_status")
            .and_then(Value::as_str)
            .unwrap_or("pending")
            .to_ascii_lowercase();
        assert!(
            verification_status == "verified",
            "committed real_world metadata must be submission-ready with verification_status=verified: {}",
            metadata_path.display()
        );
    }
}

#[test]
fn committed_real_world_provenance_status_is_submission_ready() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    let status = collect_real_world_provenance_status(common::corpus_root(), &manifest)
        .expect("provenance status summary should be computed for committed corpus");

    assert!(
        status.pending_contract_ids.is_empty(),
        "committed corpus should not contain pending real_world provenance entries"
    );
    assert!(
        status.verified_contracts >= MIN_REAL_WORLD_CONTRACTS,
        "committed corpus should report verified status for all real_world contracts"
    );
}

#[test]
fn committed_corpus_sources_must_span_multiple_template_shapes() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    let mut all_shapes = BTreeSet::new();
    let mut real_world_shapes = BTreeSet::new();

    for contract in &manifest.contracts {
        let path = common::corpus_root().join(&contract.rust_source);
        let source = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!(
                "failed to read committed corpus rust source {}: {err}",
                path.display()
            )
        });

        let shape = normalize_source_template_shape(&source);
        all_shapes.insert(shape.clone());
        if matches!(contract.category, CorpusCategory::RealWorld) {
            real_world_shapes.insert(shape);
        }
    }

    assert!(
        all_shapes.len() >= 4,
        "committed corpus sources should not collapse to one trivial template shape",
    );
    assert!(
        real_world_shapes.len() >= 3,
        "real_world corpus sources should include multiple authored template shapes",
    );
}

#[test]
fn committed_corpus_wasm_name_section_presence_matches_manifest_flags() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    for contract in &manifest.contracts {
        for variant in &contract.variants {
            let wasm_path = common::corpus_root().join(&variant.wasm_path);
            let bytes = fs::read(&wasm_path).unwrap_or_else(|err| {
                panic!(
                    "failed to read committed wasm fixture {}: {err}",
                    wasm_path.display()
                )
            });
            let custom_sections = list_custom_section_names(&bytes)
                .unwrap_or_else(|message| panic!("failed to scan custom sections: {message}"));
            let has_name_section = custom_sections.iter().any(|name| name == "name");

            if variant.include_debug_names {
                assert!(
                    has_name_section,
                    "expected `name` custom section for include_debug_names=true: {}",
                    wasm_path.display()
                );
            } else {
                assert!(
                    !has_name_section,
                    "unexpected `name` custom section for include_debug_names=false: {}",
                    wasm_path.display()
                );
            }
        }
    }
}

#[test]
fn committed_locked_corpus_forbids_sorcat_meta_watermark_custom_sections() {
    let manifest = load_manifest(common::corpus_manifest_path())
        .expect("manifest loader should parse fixtures/corpus/manifest.v1.json");

    assert!(
        manifest.locked,
        "this watermark guard is for the committed locked corpus"
    );

    for contract in &manifest.contracts {
        for variant in &contract.variants {
            let wasm_path = common::corpus_root().join(&variant.wasm_path);
            let bytes = fs::read(&wasm_path).unwrap_or_else(|err| {
                panic!(
                    "failed to read committed wasm fixture {}: {err}",
                    wasm_path.display()
                )
            });
            let custom_sections = list_custom_section_names(&bytes)
                .unwrap_or_else(|message| panic!("failed to scan custom sections: {message}"));

            for forbidden in ["sorcat.meta.v1"] {
                assert!(
                    !custom_sections.iter().any(|name| name == forbidden),
                    "locked corpus must not include watermark custom section `{forbidden}`: {}",
                    wasm_path.display()
                );
            }
        }
    }
}

#[test]
fn fixture_layout_validation_requires_sources_wasm_and_metadata() {
    let tmp = common::TempDir::new("valid-layout");
    let root = tmp.path();

    common::write_text(
        root.join("contracts/real_world/token/src/lib.rs"),
        "pub fn transfer() {}",
    );
    common::write_text(
        root.join("contracts/real_world/token/metadata.json"),
        r#"{
          "kind":"token",
          "source_provenance":{
            "upstream_repo_url":"https://github.com/stellar/soroban-examples",
            "upstream_commit":"4f3c2b1a9d8e7f6c5b4a3a2f1e0d9c8b7a6f5e4d",
            "upstream_license":"Apache-2.0",
            "source_origin":"upstream_open_source_contract",
            "build_recipe":"cargo build --target wasm32-unknown-unknown --release",
            "verification_status":"pending",
            "verification_note":"offline verification pending for fixture test"
          }
        }"#,
    );
    common::write_wasm_header(root.join("contracts/real_world/token/wasm/debug.wasm"));
    common::write_wasm_header(root.join("contracts/real_world/token/wasm/release.wasm"));

    let manifest_path = root.join("manifest.v1.json");
    let manifest_json = r#"
    {
      "schema_version": "1.0.0",
      "locked": true,
      "contracts": [
        {
          "id": "real_world/token",
          "category": "real_world",
          "rust_source": "contracts/real_world/token/src/lib.rs",
          "metadata_path": "contracts/real_world/token/metadata.json",
          "variants": [
            {
              "profile": "debug",
              "include_debug_names": true,
              "sdk_version": "23.0.0",
              "wasm_path": "contracts/real_world/token/wasm/debug.wasm"
            },
            {
              "profile": "release",
              "include_debug_names": false,
              "sdk_version": "23.0.0",
              "wasm_path": "contracts/real_world/token/wasm/release.wasm"
            }
          ]
        }
      ]
    }
    "#;
    common::write_text(&manifest_path, manifest_json);

    let manifest =
        load_manifest(&manifest_path).expect("manifest should be valid for fixture layout test");

    validate_corpus_layout(root, &manifest).expect(
        "corpus layout must include source, valid wasm variants, and metadata for each contract",
    );
}

#[test]
fn fixture_layout_rejects_placeholder_real_world_provenance_values() {
    let tmp = common::TempDir::new("placeholder-provenance");
    let root = tmp.path();

    common::write_text(
        root.join("contracts/real_world/token/src/lib.rs"),
        "pub fn transfer() {}",
    );
    common::write_text(
        root.join("contracts/real_world/token/metadata.json"),
        r#"{
          "id":"real_world/token",
          "source_provenance":{
            "upstream_repo_url":"https://example.invalid/project",
            "upstream_commit":"locked-corpus-v1-seq-01",
            "upstream_license":"Apache-2.0",
            "source_origin":"curated_fixture_seed",
            "build_recipe":"todo"
          }
        }"#,
    );
    common::write_wasm_header(root.join("contracts/real_world/token/wasm/debug.wasm"));

    let manifest_path = root.join("manifest.v1.json");
    common::write_text(
        &manifest_path,
        r#"{
          "schema_version":"1.0.0",
          "locked":false,
          "contracts":[
            {
              "id":"real_world/token",
              "category":"real_world",
              "rust_source":"contracts/real_world/token/src/lib.rs",
              "metadata_path":"contracts/real_world/token/metadata.json",
              "variants":[
                {
                  "profile":"debug",
                  "include_debug_names":true,
                  "sdk_version":"25.0.1",
                  "wasm_path":"contracts/real_world/token/wasm/debug.wasm"
                }
              ]
            }
          ]
        }"#,
    );

    let manifest = load_manifest(&manifest_path).expect("manifest should parse for fixture");
    let error = validate_corpus_layout(root, &manifest)
        .expect_err("placeholder provenance values must be rejected");
    assert_eq!(error.kind(), EvalErrorKind::InvalidManifest);
}

#[test]
fn manifest_loader_rejects_duplicate_contract_ids_with_invalid_manifest_error() {
    let tmp = common::TempDir::new("duplicate-contract-id");
    let manifest_path = tmp.path().join("manifest.v1.json");
    let manifest_json = r#"
    {
      "schema_version": "1.0.0",
      "locked": true,
      "contracts": [
        {
          "id": "real_world/token",
          "category": "real_world",
          "rust_source": "contracts/real_world/token/src/lib.rs",
          "metadata_path": "contracts/real_world/token/metadata.json",
          "variants": [
            {
              "profile": "debug",
              "include_debug_names": true,
              "sdk_version": "23.0.0",
              "wasm_path": "contracts/real_world/token/wasm/debug.wasm"
            }
          ]
        },
        {
          "id": "real_world/token",
          "category": "synthetic",
          "rust_source": "contracts/synthetic/token/src/lib.rs",
          "metadata_path": "contracts/synthetic/token/metadata.json",
          "variants": [
            {
              "profile": "release",
              "include_debug_names": false,
              "sdk_version": "22.1.0",
              "wasm_path": "contracts/synthetic/token/wasm/release.wasm"
            }
          ]
        }
      ]
    }
    "#;
    common::write_text(&manifest_path, manifest_json);

    let err = load_manifest(&manifest_path)
        .expect_err("duplicate contract IDs must be rejected at manifest load time");
    assert_eq!(err.kind(), EvalErrorKind::InvalidManifest);
}

fn semantic_fingerprint(wasm: &[u8]) -> String {
    let summary = decode_module_summary(wasm).expect("semantic fingerprint decode should succeed");

    let mut import_parts = Vec::with_capacity(summary.import_functions.len());
    for import in &summary.import_functions {
        import_parts.push(format!(
            "{}::{}({})->({})",
            import.module,
            import.name,
            import.params.join(","),
            import.results.join(",")
        ));
    }

    let mut export_parts = Vec::with_capacity(summary.exports.len());
    for export in &summary.exports {
        export_parts.push(format!(
            "{:?}::{}::{}",
            export.kind, export.name, export.index
        ));
    }

    let mut body_parts = Vec::with_capacity(summary.function_bodies.len());
    for body in &summary.function_bodies {
        let instructions = body
            .instructions
            .iter()
            .map(fingerprint_instruction)
            .collect::<Vec<_>>()
            .join(",");
        body_parts.push(format!("{}:{}", body.symbol, instructions));
    }

    format!(
        "imports=[{}]|exports=[{}]|bodies=[{}]",
        import_parts.join(";"),
        export_parts.join(";"),
        body_parts.join(";")
    )
}

fn fingerprint_instruction(instruction: &Instruction) -> String {
    match instruction {
        Instruction::Block => "block".to_string(),
        Instruction::Loop => "loop".to_string(),
        Instruction::If => "if".to_string(),
        Instruction::Else => "else".to_string(),
        Instruction::End => "end".to_string(),
        Instruction::Br { depth } => format!("br {depth}"),
        Instruction::BrIf { depth } => format!("br_if {depth}"),
        Instruction::Call { function_index } => format!("call {function_index}"),
        Instruction::CallIndirect {
            type_index,
            table_index,
        } => format!("call_indirect {type_index} {table_index}"),
        Instruction::LocalGet { local_index } => format!("local.get {local_index}"),
        Instruction::LocalSet { local_index } => format!("local.set {local_index}"),
        Instruction::LocalTee { local_index } => format!("local.tee {local_index}"),
        Instruction::GlobalGet { global_index } => format!("global.get {global_index}"),
        Instruction::GlobalSet { global_index } => format!("global.set {global_index}"),
        Instruction::I32Const { value } => format!("i32.const {value}"),
        Instruction::I64Const { value } => format!("i64.const {value}"),
        Instruction::I32Eq => "i32.eq".to_string(),
        Instruction::I32Ne => "i32.ne".to_string(),
        Instruction::I32LtS => "i32.lt_s".to_string(),
        Instruction::I32GtS => "i32.gt_s".to_string(),
        Instruction::I32Eqz => "i32.eqz".to_string(),
        Instruction::I64Eqz => "i64.eqz".to_string(),
        Instruction::I64Eq => "i64.eq".to_string(),
        Instruction::I64Ne => "i64.ne".to_string(),
        Instruction::I64LtS => "i64.lt_s".to_string(),
        Instruction::I64GtS => "i64.gt_s".to_string(),
        Instruction::I32Add => "i32.add".to_string(),
        Instruction::I32Sub => "i32.sub".to_string(),
        Instruction::I32Mul => "i32.mul".to_string(),
        Instruction::I64Add => "i64.add".to_string(),
        Instruction::I64Sub => "i64.sub".to_string(),
        Instruction::I64Mul => "i64.mul".to_string(),
        Instruction::Select => "select".to_string(),
        Instruction::BrTable {
            targets,
            default_target,
        } => format!("br_table {:?} {default_target}", targets),
        Instruction::Drop => "drop".to_string(),
        Instruction::Return => "return".to_string(),
        _ => format!("{instruction:?}"),
    }
}

fn list_custom_section_names(wasm: &[u8]) -> Result<Vec<String>, String> {
    if wasm.len() < 8 || &wasm[0..4] != b"\0asm" || &wasm[4..8] != &[0x01, 0x00, 0x00, 0x00] {
        return Err("invalid wasm header".to_string());
    }

    let mut offset = 8usize;
    let mut names = Vec::new();

    while offset < wasm.len() {
        let section_id = read_u8(wasm, &mut offset)?;
        let section_size = read_var_u32(wasm, &mut offset)? as usize;
        let end = offset
            .checked_add(section_size)
            .ok_or_else(|| "section length overflow".to_string())?;
        if end > wasm.len() {
            return Err("section extends past EOF".to_string());
        }

        if section_id == 0 {
            let mut custom_offset = offset;
            let name_len = read_var_u32(wasm, &mut custom_offset)? as usize;
            let name_end = custom_offset
                .checked_add(name_len)
                .ok_or_else(|| "custom section name length overflow".to_string())?;
            if name_end > end {
                return Err("custom section name extends past section".to_string());
            }
            let name_bytes = &wasm[custom_offset..name_end];
            let name = std::str::from_utf8(name_bytes)
                .map_err(|_| "custom section name is not utf-8".to_string())?
                .to_string();
            names.push(name);
        }

        offset = end;
    }

    Ok(names)
}

fn read_u8(bytes: &[u8], offset: &mut usize) -> Result<u8, String> {
    let value = *bytes
        .get(*offset)
        .ok_or_else(|| "unexpected EOF".to_string())?;
    *offset += 1;
    Ok(value)
}

fn read_var_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, String> {
    let mut result = 0u32;
    let mut shift = 0u32;

    loop {
        let byte = read_u8(bytes, offset)?;
        result |= u32::from(byte & 0x7f)
            .checked_shl(shift)
            .ok_or_else(|| "varint shift overflow".to_string())?;

        if (byte & 0x80) == 0 {
            return Ok(result);
        }

        shift += 7;
        if shift >= 35 {
            return Err("invalid varint".to_string());
        }
    }
}

fn normalize_source_template_shape(source: &str) -> String {
    let mut normalized = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut idx = 0usize;

    while idx < bytes.len() {
        let ch = bytes[idx] as char;

        if ch == '"' {
            normalized.push('"');
            normalized.push_str("str");
            normalized.push('"');
            idx += 1;
            while idx < bytes.len() {
                let c = bytes[idx] as char;
                idx += 1;
                if c == '\\' {
                    idx += 1;
                    continue;
                }
                if c == '"' {
                    break;
                }
            }
            continue;
        }

        if ch.is_ascii_alphanumeric() || ch == '_' {
            let start = idx;
            idx += 1;
            while idx < bytes.len() {
                let c = bytes[idx] as char;
                if c.is_ascii_alphanumeric() || c == '_' {
                    idx += 1;
                } else {
                    break;
                }
            }
            let token = &source[start..idx];
            if is_shape_keyword(token) {
                normalized.push_str(token);
            } else {
                normalized.push_str("id");
            }
            normalized.push(' ');
            continue;
        }

        if ch.is_ascii_whitespace() {
            if !normalized.ends_with(' ') {
                normalized.push(' ');
            }
            idx += 1;
            continue;
        }

        normalized.push(ch);
        idx += 1;
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_shape_keyword(token: &str) -> bool {
    matches!(
        token,
        "#" | "no_std"
            | "use"
            | "core"
            | "panic"
            | "PanicInfo"
            | "panic_handler"
            | "fn"
            | "loop"
            | "link"
            | "wasm_import_module"
            | "extern"
            | "C"
            | "link_name"
            | "no_mangle"
            | "pub"
            | "const"
            | "unsafe"
            | "let"
            | "mod"
            | "super"
            | "i64"
            | "str"
    )
}

#[test]
fn fixture_layout_validation_rejects_non_wasm_payload_with_invalid_manifest_error() {
    let tmp = common::TempDir::new("invalid-wasm");
    let root = tmp.path();

    common::write_text(
        root.join("contracts/adversarial/fuzz/src/lib.rs"),
        "pub fn fuzz() {}",
    );
    common::write_text(
        root.join("contracts/adversarial/fuzz/metadata.json"),
        r#"{"kind":"adversarial"}"#,
    );
    common::write_text(
        root.join("contracts/adversarial/fuzz/wasm/debug.wasm"),
        "not-a-wasm",
    );

    let manifest = sorcat_eval::corpus::CorpusManifest {
        schema_version: "1.0.0".to_string(),
        locked: true,
        contracts: vec![sorcat_eval::corpus::CorpusContractEntry {
            id: "adversarial/fuzz".to_string(),
            category: CorpusCategory::Adversarial,
            rust_source: "contracts/adversarial/fuzz/src/lib.rs".into(),
            metadata_path: "contracts/adversarial/fuzz/metadata.json".into(),
            variants: vec![sorcat_eval::corpus::BuildVariant {
                profile: BuildProfile::Debug,
                include_debug_names: true,
                sdk_version: "23.0.0".to_string(),
                wasm_path: "contracts/adversarial/fuzz/wasm/debug.wasm".into(),
            }],
        }],
    };

    let err = validate_corpus_layout(root, &manifest)
        .expect_err("invalid wasm bytes must fail corpus layout validation");
    assert_eq!(err.kind(), EvalErrorKind::InvalidManifest);
}

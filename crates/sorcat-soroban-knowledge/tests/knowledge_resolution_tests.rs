use sorcat_soroban_knowledge::{SorobanSymbolKind, classify_import, resolve_imports};

#[test]
fn classifies_env_builtin_import() {
    assert_eq!(
        classify_import("env", "vec_new"),
        SorobanSymbolKind::EnvBuiltin,
    );
}

#[test]
fn classifies_unknown_env_import() {
    assert_eq!(
        classify_import("env", "future_host_fn"),
        SorobanSymbolKind::EnvUnknown,
    );
}

#[test]
fn classifies_non_env_import_as_non_env() {
    assert_eq!(
        classify_import("not_env", "helper"),
        SorobanSymbolKind::NonEnv,
    );
}

#[test]
fn resolves_in_deterministic_lexicographic_order() {
    let resolved = resolve_imports([
        ("env", "mystery_host_fn"),
        ("not_env", "helper"),
        ("env", "map_new"),
        ("env", "vec_new"),
    ]);

    let rendered: Vec<String> = resolved
        .iter()
        .map(|item| format!("{}::{}", item.module, item.name))
        .collect();
    assert_eq!(
        rendered,
        vec![
            "env::map_new".to_owned(),
            "env::mystery_host_fn".to_owned(),
            "env::vec_new".to_owned(),
            "not_env::helper".to_owned(),
        ],
    );
}

#[test]
fn resolves_builtin_with_signature_protocol_confidence_and_reason() {
    let resolved = resolve_imports([("env", "vec_new")]);
    let entry = resolved
        .into_iter()
        .next()
        .expect("single input should produce single resolution");

    assert_eq!(entry.kind, SorobanSymbolKind::EnvBuiltin);
    assert!(
        entry.canonical_id.is_some(),
        "builtin resolution should include canonical id"
    );
    assert!(
        entry.signature.is_some(),
        "builtin resolution should include function signature"
    );
    assert!(
        entry.min_protocol.is_some() && entry.max_protocol.is_some(),
        "builtin resolution should include protocol window constraints"
    );
    assert!(
        entry.min_protocol.unwrap_or_default() <= entry.max_protocol.unwrap_or_default(),
        "protocol constraints should form a valid range"
    );
    assert!(
        entry.confidence >= 90,
        "exact builtin resolution should carry high confidence"
    );
    assert!(
        entry.reason.contains("exact"),
        "resolution reason should explain exact builtin matching"
    );
}

#[test]
fn marks_helpers_and_xdr_related_semantics_for_object_codecs() {
    let resolved = resolve_imports([("env", "obj_from_u64")]);
    let entry = resolved
        .into_iter()
        .next()
        .expect("single input should produce single resolution");

    assert_eq!(entry.kind, SorobanSymbolKind::EnvBuiltin);
    assert!(
        entry.semantic_tags.iter().any(|tag| tag == "env_helper"),
        "object codec builtins should be marked as helpers"
    );
    assert!(
        entry.semantic_tags.iter().any(|tag| tag == "xdr_semantic"),
        "object codec builtins should be marked with xdr semantics"
    );
    assert!(
        entry.semantic_tags.iter().any(|tag| tag == "xdr_codec"),
        "object codec builtins should be marked as xdr codecs"
    );
}

#[test]
fn unknown_env_import_has_low_confidence_with_explicit_reason() {
    let resolved = resolve_imports([("env", "future_host_fn")]);
    let entry = resolved
        .into_iter()
        .next()
        .expect("single input should produce single resolution");

    assert_eq!(entry.kind, SorobanSymbolKind::EnvUnknown);
    assert_eq!(entry.canonical_id, None);
    assert!(entry.confidence <= 30);
    assert!(
        entry.reason.contains("unresolved"),
        "unknown env imports should include explicit unresolved reason"
    );
}

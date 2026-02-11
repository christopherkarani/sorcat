use sorcat_eval::ast::{
    NormalizationOptions, normalize_original_rust, normalize_reconstructed_rust,
};

#[test]
fn normalization_canonicalizes_identifiers_for_semantically_equivalent_functions() {
    let original = r#"
        pub fn transfer(amount: i128) -> i128 {
            let fee: i128 = 1;
            amount - fee
        }
    "#;

    let reconstructed = r#"
        pub fn transfer(arg0: i128)->i128{
            let v0: i128 = 1;
            arg0 - v0
        }
    "#;

    let options = NormalizationOptions::default();

    let normalized_original =
        normalize_original_rust(original, &options).expect("normalization of original source");
    let normalized_reconstructed = normalize_reconstructed_rust(reconstructed, &options)
        .expect("normalization of reconstructed source");

    assert_eq!(
        normalized_original.canonical_source, normalized_reconstructed.canonical_source,
        "identifier canonicalization should make semantically equivalent sources match"
    );
    assert!(
        normalized_original.node_count > 0,
        "normalized AST should report a non-zero node count"
    );
}

#[test]
fn normalization_strips_formatting_noise_for_deterministic_ast_inputs() {
    let noisy = "pub   fn  f ( a : i32 )-> i32 {a+1}";
    let clean = "pub fn f(a: i32) -> i32 { a + 1 }";

    let options = NormalizationOptions {
        canonicalize_identifiers: false,
        normalize_whitespace: true,
    };

    let noisy_normalized =
        normalize_original_rust(noisy, &options).expect("normalization of noisy source");
    let clean_normalized =
        normalize_original_rust(clean, &options).expect("normalization of clean source");

    assert_eq!(
        noisy_normalized.canonical_source, clean_normalized.canonical_source,
        "whitespace normalization should produce deterministic canonical text"
    );
    assert_eq!(
        noisy_normalized.node_count, clean_normalized.node_count,
        "node_count should align to parsed AST structure instead of token formatting differences"
    );
}

#[test]
fn normalization_supports_rust_lifetimes_without_literal_parse_failures() {
    let source = r#"
        pub fn fixture_contract_id() -> &'static str {
            "real_world/asset_vault_v1"
        }
    "#;

    let normalized = normalize_original_rust(source, &NormalizationOptions::default())
        .expect("normalization should support Rust lifetime tokens like 'static");

    assert!(
        normalized.canonical_source.contains("'static"),
        "normalized output should preserve lifetime type tokens"
    );
}

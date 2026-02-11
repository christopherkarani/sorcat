use sorcat_eval::{
    EvalErrorKind,
    ast::{
        NormalizationOptions, NormalizedAst, normalize_original_rust, normalize_reconstructed_rust,
    },
    scoring::{AstScore, ContractScore, compute_ast_score, compute_coverage},
};

#[test]
fn ast_score_uses_ast_operator_nodes_for_tree_edit_distance() {
    let options = NormalizationOptions::default();
    let original = normalize_original_rust("pub fn eval(a: i32, b: i32) -> i32 { a + b }", &options)
        .expect("original source should normalize");
    let reconstructed =
        normalize_reconstructed_rust("pub fn eval(a: i32, b: i32) -> i32 { a - b }", &options)
            .expect("reconstructed source should normalize");

    let score = compute_ast_score(&original, &reconstructed).expect(
        "scoring should compute deterministic AST distance from normalized Rust AST structures",
    );

    assert_eq!(score.max_node_count, original.node_count);
    assert_eq!(score.tree_edit_distance, 1);
    assert_eq!(
        score.score,
        1.0 - (score.tree_edit_distance as f64 / score.max_node_count as f64)
    );
}

#[test]
fn ast_score_penalizes_structural_changes_more_than_single_operator_swap() {
    let options = NormalizationOptions::default();
    let original = normalize_original_rust("pub fn eval(a: i32, b: i32) -> i32 { a + b }", &options)
        .expect("original source should normalize");

    let operator_change =
        normalize_reconstructed_rust("pub fn eval(a: i32, b: i32) -> i32 { a - b }", &options)
            .expect("reconstructed source should normalize");
    let structural_change = normalize_reconstructed_rust(
        "pub fn eval(a: i32, b: i32) -> i32 { if a > b { a } else { b } }",
        &options,
    )
    .expect("reconstructed source should normalize");

    let operator_score =
        compute_ast_score(&original, &operator_change).expect("operator delta score should work");
    let structural_score = compute_ast_score(&original, &structural_change)
        .expect("structural delta score should work");

    assert!(
        structural_score.tree_edit_distance > operator_score.tree_edit_distance,
        "AST structural edits should score as larger distance than a single operator rename"
    );
    assert!(
        structural_score.score < operator_score.score,
        "larger AST structural distance must lower the reconstruction score"
    );
}

#[test]
fn ast_score_rejects_node_count_mismatch_with_structured_error() {
    let options = NormalizationOptions::default();
    let original = normalize_original_rust("pub fn eval() -> i32 { 1 }", &options)
        .expect("source should normalize");
    let mut reconstructed =
        normalize_reconstructed_rust("pub fn eval() -> i32 { 1 }", &options)
            .expect("source should normalize");
    reconstructed.node_count += 1;

    let error = compute_ast_score(&original, &reconstructed)
        .expect_err("node-count mismatch must produce a structured invalid input error");
    assert_eq!(error.kind(), EvalErrorKind::InvalidInput);
    assert!(
        error.to_string().contains("node_count")
            && error.to_string().contains("does not match parsed AST node count"),
        "error should explain node-count and parsed AST mismatch"
    );
}

#[test]
fn ast_score_returns_structured_error_for_malformed_normalized_source() {
    let options = NormalizationOptions::default();
    let original = normalize_original_rust("pub fn eval() -> i32 { 1 }", &options)
        .expect("source should normalize");
    let malformed = NormalizedAst {
        canonical_source: "pub fn broken( {".to_string(),
        node_count: 1,
    };

    let error = compute_ast_score(&original, &malformed)
        .expect_err("malformed canonical source must return invalid input error");
    assert_eq!(error.kind(), EvalErrorKind::InvalidInput);
    assert!(
        error.to_string().contains("reconstructed canonical source")
            && error.to_string().contains("not valid Rust syntax"),
        "error should preserve structured field and parsing context"
    );
}

#[test]
fn builtin_coverage_aggregates_contract_hits_and_totals() {
    let contract_scores = vec![
        ContractScore {
            contract_id: "real_world_token".to_string(),
            ast_score: AstScore {
                tree_edit_distance: 1,
                max_node_count: 100,
                score: 0.99,
            },
            builtin_hits: 48,
            builtin_total: 50,
        },
        ContractScore {
            contract_id: "synthetic_storage".to_string(),
            ast_score: AstScore {
                tree_edit_distance: 2,
                max_node_count: 100,
                score: 0.98,
            },
            builtin_hits: 49,
            builtin_total: 50,
        },
    ];

    let coverage = compute_coverage(&contract_scores)
        .expect("coverage should aggregate builtin hits across contracts");

    assert_eq!(coverage.builtin_hits, 97);
    assert_eq!(coverage.builtin_total, 100);
    assert!(
        (coverage.ratio - 0.97).abs() < f64::EPSILON,
        "coverage ratio should be computed deterministically from aggregate totals"
    );
}

#[test]
fn structurally_different_code_cannot_score_artificially_perfect() {
    let options = NormalizationOptions::default();
    let original = normalize_original_rust(
        r#"
        pub fn entry_contract() -> i64 {
            helper(7)
        }
        fn helper(value: i64) -> i64 {
            value + 1
        }
        "#,
        &options,
    )
    .expect("original source should normalize");
    let reconstructed = normalize_reconstructed_rust(
        r#"
        pub fn entry_contract() -> i64 {
            7
        }
        "#,
        &options,
    )
    .expect("reconstructed source should normalize");

    let score = compute_ast_score(&original, &reconstructed)
        .expect("score computation should succeed for valid normalized ASTs");
    assert!(
        score.tree_edit_distance > 0,
        "structural differences must produce non-zero edit distance"
    );
    assert!(
        score.score < 1.0,
        "structural differences must not be scored as perfect reconstruction"
    );
}

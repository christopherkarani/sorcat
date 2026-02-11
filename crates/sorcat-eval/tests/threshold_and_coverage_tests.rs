use sorcat_eval::{
    EvalErrorKind,
    scoring::{
        AstScore, ContractScore, CoverageMetrics, EvaluationSummary, Thresholds,
        evaluate_thresholds, summarize,
    },
};

#[test]
fn corpus_summary_must_clear_accuracy_and_builtin_coverage_targets() {
    let contract_scores = vec![
        ContractScore {
            contract_id: "contract_1".to_string(),
            ast_score: AstScore {
                tree_edit_distance: 5,
                max_node_count: 100,
                score: 0.95,
            },
            builtin_hits: 98,
            builtin_total: 100,
        },
        ContractScore {
            contract_id: "contract_2".to_string(),
            ast_score: AstScore {
                tree_edit_distance: 10,
                max_node_count: 100,
                score: 0.90,
            },
            builtin_hits: 98,
            builtin_total: 100,
        },
    ];

    let summary = summarize(&contract_scores).expect("summary aggregation should succeed");

    assert!(
        summary.mean_ast_score >= 0.90,
        "plan requires >= 0.90 mean AST reconstruction score"
    );
    assert!(
        summary.builtin_coverage.ratio >= 0.98,
        "plan requires >= 0.98 Soroban builtin/env/XDR reconstruction coverage"
    );

    evaluate_thresholds(
        &summary,
        &Thresholds {
            min_mean_ast_score: 0.90,
            min_builtin_coverage: 0.98,
        },
    )
    .expect("threshold checks should pass when summary metrics satisfy plan gates");
}

#[test]
fn threshold_gate_rejects_summary_below_plan_requirements() {
    let failing_summary = EvaluationSummary {
        contract_scores: vec![],
        mean_ast_score: 0.87,
        builtin_coverage: CoverageMetrics {
            builtin_hits: 95,
            builtin_total: 100,
            ratio: 0.95,
        },
    };

    let err = evaluate_thresholds(
        &failing_summary,
        &Thresholds {
            min_mean_ast_score: 0.90,
            min_builtin_coverage: 0.98,
        },
    )
    .expect_err("threshold checks must fail when summary metrics are below plan gates");

    assert_eq!(err.kind(), EvalErrorKind::ThresholdNotMet);
    let message = err.to_string();
    assert!(
        message.contains("0.90") || message.contains("0.98"),
        "error should explain which threshold was not met"
    );
}

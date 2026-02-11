use indexmap::IndexMap;
use sorcat_eval::{
    report::{DeterministicReport, parse_deterministic_report, render_deterministic_report},
    scoring::{CoverageMetrics, EvaluationSummary},
};

#[test]
fn report_renderer_is_deterministic_for_identical_input() {
    let mut metadata = IndexMap::new();
    metadata.insert("sdk_versions".to_string(), "22.0.0,23.0.0".to_string());
    metadata.insert("profiles".to_string(), "debug,release".to_string());
    metadata.insert("name_sections".to_string(), "with,without".to_string());

    let report = DeterministicReport {
        corpus_revision: "v1-locked".to_string(),
        summary: EvaluationSummary {
            contract_scores: vec![],
            mean_ast_score: 0.91,
            builtin_coverage: CoverageMetrics {
                builtin_hits: 98,
                builtin_total: 100,
                ratio: 0.98,
            },
        },
        metadata,
    };

    let first = render_deterministic_report(&report)
        .expect("report rendering should produce deterministic JSON output");
    let second = render_deterministic_report(&report)
        .expect("report rendering should produce deterministic JSON output");

    assert_eq!(first, second, "report output must be deterministic for CI");
}

#[test]
fn report_json_contains_required_ci_fields() {
    let mut metadata = IndexMap::new();
    metadata.insert("corpus_size".to_string(), "40".to_string());

    let report = DeterministicReport {
        corpus_revision: "v1-locked".to_string(),
        summary: EvaluationSummary {
            contract_scores: vec![],
            mean_ast_score: 0.90,
            builtin_coverage: CoverageMetrics {
                builtin_hits: 98,
                builtin_total: 100,
                ratio: 0.98,
            },
        },
        metadata,
    };

    let json =
        render_deterministic_report(&report).expect("report rendering should return valid JSON");
    let parsed = parse_deterministic_report(&json).expect("rendered report should parse as JSON");

    assert_eq!(parsed["corpus_revision"], "v1-locked");
    assert_eq!(parsed["summary"]["mean_ast_score"], 0.90);
    assert_eq!(parsed["summary"]["builtin_coverage"]["ratio"], 0.98);
    assert_eq!(parsed["metadata"]["corpus_size"], "40");
}

#[test]
fn report_renderer_is_deterministic_for_different_metadata_insertion_orders() {
    let mut metadata_a = IndexMap::new();
    metadata_a.insert("zeta".to_string(), "3".to_string());
    metadata_a.insert("alpha".to_string(), "1".to_string());
    metadata_a.insert("middle".to_string(), "2".to_string());

    let mut metadata_b = IndexMap::new();
    metadata_b.insert("middle".to_string(), "2".to_string());
    metadata_b.insert("zeta".to_string(), "3".to_string());
    metadata_b.insert("alpha".to_string(), "1".to_string());

    let report_a = DeterministicReport {
        corpus_revision: "v1-locked".to_string(),
        summary: EvaluationSummary {
            contract_scores: vec![],
            mean_ast_score: 0.92,
            builtin_coverage: CoverageMetrics {
                builtin_hits: 99,
                builtin_total: 100,
                ratio: 0.99,
            },
        },
        metadata: metadata_a,
    };

    let report_b = DeterministicReport {
        corpus_revision: "v1-locked".to_string(),
        summary: EvaluationSummary {
            contract_scores: vec![],
            mean_ast_score: 0.92,
            builtin_coverage: CoverageMetrics {
                builtin_hits: 99,
                builtin_total: 100,
                ratio: 0.99,
            },
        },
        metadata: metadata_b,
    };

    let rendered_a =
        render_deterministic_report(&report_a).expect("report rendering should succeed");
    let rendered_b =
        render_deterministic_report(&report_b).expect("report rendering should succeed");

    assert_eq!(
        rendered_a, rendered_b,
        "metadata insertion order must not change canonical report output"
    );
}

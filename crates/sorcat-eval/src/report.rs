use std::path::PathBuf;

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::{EvalError, scoring::EvaluationSummary};

#[derive(Debug, Clone, PartialEq)]
pub struct DeterministicReport {
    pub corpus_revision: String,
    pub summary: EvaluationSummary,
    pub metadata: IndexMap<String, String>,
}

pub fn render_deterministic_report(report: &DeterministicReport) -> Result<String, EvalError> {
    if report.corpus_revision.trim().is_empty() {
        return Err(EvalError::EmptyReport);
    }

    if !report.summary.mean_ast_score.is_finite()
        || !report.summary.builtin_coverage.ratio.is_finite()
    {
        return Err(EvalError::InvalidInput {
            field: "summary",
            message: "summary metrics must be finite".to_string(),
        });
    }

    let mut sorted_contract_scores = report.summary.contract_scores.clone();
    sorted_contract_scores.sort_by(|left, right| left.contract_id.cmp(&right.contract_id));

    let contract_scores_json: Vec<Value> = sorted_contract_scores
        .into_iter()
        .map(|contract| {
            let mut ast_score = Map::new();
            ast_score.insert(
                "max_node_count".to_string(),
                Value::from(contract.ast_score.max_node_count as u64),
            );
            ast_score.insert("score".to_string(), Value::from(contract.ast_score.score));
            ast_score.insert(
                "tree_edit_distance".to_string(),
                Value::from(contract.ast_score.tree_edit_distance as u64),
            );

            let mut contract_map = Map::new();
            contract_map.insert("ast_score".to_string(), Value::Object(ast_score));
            contract_map.insert(
                "builtin_hits".to_string(),
                Value::from(contract.builtin_hits as u64),
            );
            contract_map.insert(
                "builtin_total".to_string(),
                Value::from(contract.builtin_total as u64),
            );
            contract_map.insert(
                "contract_id".to_string(),
                Value::String(contract.contract_id),
            );
            Value::Object(contract_map)
        })
        .collect();

    let mut builtin_coverage = Map::new();
    builtin_coverage.insert(
        "builtin_hits".to_string(),
        Value::from(report.summary.builtin_coverage.builtin_hits as u64),
    );
    builtin_coverage.insert(
        "builtin_total".to_string(),
        Value::from(report.summary.builtin_coverage.builtin_total as u64),
    );
    builtin_coverage.insert(
        "ratio".to_string(),
        Value::from(report.summary.builtin_coverage.ratio),
    );

    let mut summary = Map::new();
    summary.insert(
        "builtin_coverage".to_string(),
        Value::Object(builtin_coverage),
    );
    summary.insert(
        "contract_scores".to_string(),
        Value::Array(contract_scores_json),
    );
    summary.insert(
        "mean_ast_score".to_string(),
        Value::from(report.summary.mean_ast_score),
    );

    let mut metadata = Map::new();
    let mut metadata_entries: Vec<_> = report.metadata.iter().collect();
    metadata_entries.sort_by(|left, right| left.0.cmp(right.0));
    for (key, value) in metadata_entries {
        if key.trim().is_empty() {
            return Err(EvalError::InvalidInput {
                field: "metadata",
                message: "metadata keys cannot be empty".to_string(),
            });
        }
        metadata.insert(key.clone(), Value::String(value.clone()));
    }

    let mut root = Map::new();
    root.insert(
        "corpus_revision".to_string(),
        Value::String(report.corpus_revision.clone()),
    );
    root.insert("summary".to_string(), Value::Object(summary));
    root.insert("metadata".to_string(), Value::Object(metadata));

    let canonicalized = canonicalize_json(Value::Object(root));
    serde_json::to_string(&canonicalized).map_err(|source| EvalError::Json {
        path: PathBuf::from("<inline-report>"),
        source,
    })
}

pub fn parse_deterministic_report(json_report: &str) -> Result<serde_json::Value, EvalError> {
    let path = PathBuf::from("<inline-report>");
    serde_json::from_str(json_report).map_err(|source| EvalError::Json { path, source })
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted_entries: Vec<_> = map.into_iter().collect();
            sorted_entries.sort_by(|left, right| left.0.cmp(&right.0));

            let mut canonical = Map::new();
            for (key, value) in sorted_entries {
                canonical.insert(key, canonicalize_json(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        other => other,
    }
}

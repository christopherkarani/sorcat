use crate::{
    EvalError,
    ast::{AstTree, NormalizedAst, parse_rust_ast_to_tree},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct AstScore {
    pub tree_edit_distance: usize,
    pub max_node_count: usize,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractScore {
    pub contract_id: String,
    pub ast_score: AstScore,
    pub builtin_hits: usize,
    pub builtin_total: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoverageMetrics {
    pub builtin_hits: usize,
    pub builtin_total: usize,
    pub ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationSummary {
    pub contract_scores: Vec<ContractScore>,
    pub mean_ast_score: f64,
    pub builtin_coverage: CoverageMetrics,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Thresholds {
    pub min_mean_ast_score: f64,
    pub min_builtin_coverage: f64,
}

/// Maximum number of AST nodes permitted per tree in tree edit distance
/// computation. Beyond this limit, the O(n²) memory allocation becomes
/// prohibitive (e.g. 10k nodes → ~800MB). Inputs exceeding this limit are
/// rejected with `EvalError::InvalidInput`.
pub const MAX_AST_NODE_COUNT: usize = 5_000;

pub fn compute_ast_score(
    original: &NormalizedAst,
    reconstructed: &NormalizedAst,
) -> Result<AstScore, EvalError> {
    validate_normalized_ast(original, "original")?;
    validate_normalized_ast(reconstructed, "reconstructed")?;

    let original_tree = parse_rust_ast_to_tree(
        &original.canonical_source,
        "normalized_ast.canonical_source",
        "original canonical source",
    )?;
    let reconstructed_tree = parse_rust_ast_to_tree(
        &reconstructed.canonical_source,
        "normalized_ast.canonical_source",
        "reconstructed canonical source",
    )?;

    validate_node_count_alignment("original", original.node_count, original_tree.node_count())?;
    validate_node_count_alignment(
        "reconstructed",
        reconstructed.node_count,
        reconstructed_tree.node_count(),
    )?;

    if original_tree.node_count() > MAX_AST_NODE_COUNT {
        return Err(EvalError::InvalidInput {
            field: "normalized_ast.node_count",
            message: format!(
                "original AST node count ({}) exceeds maximum allowed ({MAX_AST_NODE_COUNT})",
                original_tree.node_count()
            ),
        });
    }
    if reconstructed_tree.node_count() > MAX_AST_NODE_COUNT {
        return Err(EvalError::InvalidInput {
            field: "normalized_ast.node_count",
            message: format!(
                "reconstructed AST node count ({}) exceeds maximum allowed ({MAX_AST_NODE_COUNT})",
                reconstructed_tree.node_count()
            ),
        });
    }

    let max_node_count = original_tree.node_count().max(reconstructed_tree.node_count());
    let tree_edit_distance =
        ordered_tree_edit_distance(&original_tree, &reconstructed_tree).min(max_node_count);
    let score = 1.0 - (tree_edit_distance as f64 / max_node_count as f64);

    Ok(AstScore {
        tree_edit_distance,
        max_node_count,
        score,
    })
}

pub fn compute_coverage(contract_scores: &[ContractScore]) -> Result<CoverageMetrics, EvalError> {
    if contract_scores.is_empty() {
        return Err(EvalError::InvalidInput {
            field: "contract_scores",
            message: "at least one contract score is required".to_string(),
        });
    }

    let mut seen = BTreeSet::new();
    let mut total_hits = 0usize;
    let mut total_builtin = 0usize;

    for contract in contract_scores {
        validate_contract_score(contract)?;

        if !seen.insert(contract.contract_id.clone()) {
            return Err(EvalError::InvalidInput {
                field: "contract_scores.contract_id",
                message: format!("duplicate contract id `{}`", contract.contract_id),
            });
        }

        total_hits = total_hits
            .checked_add(contract.builtin_hits)
            .ok_or_else(|| EvalError::InvalidInput {
                field: "contract_scores.builtin_hits",
                message: "builtin hit aggregation overflowed usize".to_string(),
            })?;
        total_builtin = total_builtin
            .checked_add(contract.builtin_total)
            .ok_or_else(|| EvalError::InvalidInput {
                field: "contract_scores.builtin_total",
                message: "builtin total aggregation overflowed usize".to_string(),
            })?;
    }

    if total_builtin == 0 {
        return Err(EvalError::InvalidInput {
            field: "contract_scores.builtin_total",
            message: "aggregate builtin_total must be greater than zero".to_string(),
        });
    }

    Ok(CoverageMetrics {
        builtin_hits: total_hits,
        builtin_total: total_builtin,
        ratio: total_hits as f64 / total_builtin as f64,
    })
}

pub fn summarize(contract_scores: &[ContractScore]) -> Result<EvaluationSummary, EvalError> {
    if contract_scores.is_empty() {
        return Err(EvalError::InvalidInput {
            field: "contract_scores",
            message: "cannot summarize an empty set of contracts".to_string(),
        });
    }

    let mut ordered_scores = contract_scores.to_vec();
    ordered_scores.sort_by(|left, right| left.contract_id.cmp(&right.contract_id));

    for contract in &ordered_scores {
        validate_contract_score(contract)?;
    }

    let score_sum: f64 = ordered_scores
        .iter()
        .map(|score| score.ast_score.score)
        .sum();
    let mean_ast_score = score_sum / ordered_scores.len() as f64;
    if !mean_ast_score.is_finite() {
        return Err(EvalError::InvalidInput {
            field: "contract_scores.ast_score.score",
            message: "mean AST score must be finite".to_string(),
        });
    }

    let builtin_coverage = compute_coverage(&ordered_scores)?;

    Ok(EvaluationSummary {
        contract_scores: ordered_scores,
        mean_ast_score,
        builtin_coverage,
    })
}

pub fn evaluate_thresholds(
    summary: &EvaluationSummary,
    thresholds: &Thresholds,
) -> Result<(), EvalError> {
    validate_ratio("summary.mean_ast_score", summary.mean_ast_score)?;
    validate_ratio(
        "summary.builtin_coverage.ratio",
        summary.builtin_coverage.ratio,
    )?;
    validate_ratio(
        "thresholds.min_mean_ast_score",
        thresholds.min_mean_ast_score,
    )?;
    validate_ratio(
        "thresholds.min_builtin_coverage",
        thresholds.min_builtin_coverage,
    )?;

    if summary.mean_ast_score < thresholds.min_mean_ast_score {
        return Err(EvalError::ThresholdNotMet {
            metric: "mean_ast_score",
            actual: summary.mean_ast_score,
            minimum: thresholds.min_mean_ast_score,
        });
    }

    if summary.builtin_coverage.ratio < thresholds.min_builtin_coverage {
        return Err(EvalError::ThresholdNotMet {
            metric: "builtin_coverage.ratio",
            actual: summary.builtin_coverage.ratio,
            minimum: thresholds.min_builtin_coverage,
        });
    }

    Ok(())
}

fn validate_normalized_ast(ast: &NormalizedAst, label: &'static str) -> Result<(), EvalError> {
    if ast.canonical_source.trim().is_empty() {
        return Err(EvalError::InvalidInput {
            field: "normalized_ast.canonical_source",
            message: format!("{label} canonical source cannot be empty"),
        });
    }

    if ast.node_count == 0 {
        return Err(EvalError::InvalidInput {
            field: "normalized_ast.node_count",
            message: format!("{label} node_count must be greater than zero"),
        });
    }

    Ok(())
}

fn validate_contract_score(contract: &ContractScore) -> Result<(), EvalError> {
    if contract.contract_id.trim().is_empty() {
        return Err(EvalError::InvalidInput {
            field: "contract_scores.contract_id",
            message: "contract_id cannot be empty".to_string(),
        });
    }

    if contract.builtin_hits > contract.builtin_total {
        return Err(EvalError::InvalidInput {
            field: "contract_scores.builtin_hits",
            message: format!(
                "builtin_hits ({}) cannot exceed builtin_total ({})",
                contract.builtin_hits, contract.builtin_total
            ),
        });
    }

    let ast_score = &contract.ast_score;
    if ast_score.max_node_count == 0 {
        return Err(EvalError::InvalidInput {
            field: "contract_scores.ast_score.max_node_count",
            message: "max_node_count must be greater than zero".to_string(),
        });
    }
    if ast_score.tree_edit_distance > ast_score.max_node_count {
        return Err(EvalError::InvalidInput {
            field: "contract_scores.ast_score.tree_edit_distance",
            message: "tree_edit_distance cannot exceed max_node_count".to_string(),
        });
    }
    validate_ratio("contract_scores.ast_score.score", ast_score.score)?;

    let expected_score =
        1.0 - (ast_score.tree_edit_distance as f64 / ast_score.max_node_count as f64);
    if (ast_score.score - expected_score).abs() > 1e-12 {
        return Err(EvalError::InvalidInput {
            field: "contract_scores.ast_score.score",
            message: format!(
                "ast score mismatch: got {}, expected {}",
                ast_score.score, expected_score
            ),
        });
    }

    Ok(())
}

fn validate_ratio(field: &'static str, value: f64) -> Result<(), EvalError> {
    if !value.is_finite() {
        return Err(EvalError::InvalidInput {
            field,
            message: "value must be finite".to_string(),
        });
    }

    if !(0.0..=1.0).contains(&value) {
        return Err(EvalError::InvalidInput {
            field,
            message: "value must be within [0.0, 1.0]".to_string(),
        });
    }

    Ok(())
}

fn validate_node_count_alignment(
    label: &'static str,
    provided: usize,
    parsed: usize,
) -> Result<(), EvalError> {
    if provided != parsed {
        return Err(EvalError::InvalidInput {
            field: "normalized_ast.node_count",
            message: format!(
                "{label} node_count ({provided}) does not match parsed AST node count ({parsed})"
            ),
        });
    }

    Ok(())
}

fn ordered_tree_edit_distance(left: &AstTree, right: &AstTree) -> usize {
    let left_postorder = PostorderTree::from_tree(left);
    let right_postorder = PostorderTree::from_tree(right);
    let left_count = left_postorder.node_count();
    let right_count = right_postorder.node_count();

    let mut tree_distance = vec![vec![0usize; right_count + 1]; left_count + 1];
    for &left_keyroot in &left_postorder.keyroots {
        for &right_keyroot in &right_postorder.keyroots {
            compute_forest_distance(
                left_keyroot,
                right_keyroot,
                &left_postorder,
                &right_postorder,
                &mut tree_distance,
            );
        }
    }

    tree_distance[left_count][right_count]
}

fn compute_forest_distance(
    left_root: usize,
    right_root: usize,
    left: &PostorderTree<'_>,
    right: &PostorderTree<'_>,
    tree_distance: &mut [Vec<usize>],
) {
    let left_start = left.leftmost_leaf_descendant[left_root];
    let right_start = right.leftmost_leaf_descendant[right_root];
    let row_count = left_root - left_start + 2;
    let col_count = right_root - right_start + 2;

    let mut forest_distance = vec![vec![0usize; col_count]; row_count];

    for row in 1..row_count {
        forest_distance[row][0] = forest_distance[row - 1][0] + 1;
    }
    for col in 1..col_count {
        forest_distance[0][col] = forest_distance[0][col - 1] + 1;
    }

    for row in 1..row_count {
        let left_node = left_start + row - 1;
        for col in 1..col_count {
            let right_node = right_start + col - 1;

            if left.leftmost_leaf_descendant[left_node] == left_start
                && right.leftmost_leaf_descendant[right_node] == right_start
            {
                let substitution_cost =
                    usize::from(left.labels[left_node] != right.labels[right_node]);
                let delete_cost = forest_distance[row - 1][col] + 1;
                let insert_cost = forest_distance[row][col - 1] + 1;
                let replace_cost = forest_distance[row - 1][col - 1] + substitution_cost;
                let best = delete_cost.min(insert_cost).min(replace_cost);
                forest_distance[row][col] = best;
                tree_distance[left_node][right_node] = best;
            } else {
                let delete_cost = forest_distance[row - 1][col] + 1;
                let insert_cost = forest_distance[row][col - 1] + 1;
                let left_subproblem = left.leftmost_leaf_descendant[left_node] - left_start;
                let right_subproblem = right.leftmost_leaf_descendant[right_node] - right_start;
                let replace_cost = forest_distance[left_subproblem][right_subproblem]
                    + tree_distance[left_node][right_node];
                forest_distance[row][col] = delete_cost.min(insert_cost).min(replace_cost);
            }
        }
    }
}

#[derive(Debug)]
struct PostorderTree<'a> {
    labels: Vec<&'a str>,
    leftmost_leaf_descendant: Vec<usize>,
    keyroots: Vec<usize>,
}

impl<'a> PostorderTree<'a> {
    fn from_tree(tree: &'a AstTree) -> Self {
        let mut postorder_nodes = Vec::with_capacity(tree.node_count());
        collect_postorder(tree, tree.root(), &mut postorder_nodes);

        let mut postorder_index = vec![0usize; tree.node_count()];
        for (idx, node_id) in postorder_nodes.iter().enumerate() {
            postorder_index[*node_id] = idx + 1;
        }

        let mut labels = vec![""; postorder_nodes.len() + 1];
        let mut leftmost_leaf_descendant = vec![0usize; postorder_nodes.len() + 1];
        let mut leftmost_leaf_cache = vec![None; tree.node_count()];

        for (idx, node_id) in postorder_nodes.iter().enumerate() {
            let post_idx = idx + 1;
            labels[post_idx] = tree.label(*node_id);
            let leftmost_leaf =
                leftmost_leaf_node(tree, *node_id, &mut leftmost_leaf_cache);
            leftmost_leaf_descendant[post_idx] = postorder_index[leftmost_leaf];
        }

        let mut keyroot_map = BTreeMap::new();
        for (idx, lld) in leftmost_leaf_descendant.iter().enumerate().skip(1) {
            keyroot_map.insert(*lld, idx);
        }
        let mut keyroots: Vec<usize> = keyroot_map.into_values().collect();
        keyroots.sort_unstable();

        Self {
            labels,
            leftmost_leaf_descendant,
            keyroots,
        }
    }

    fn node_count(&self) -> usize {
        self.labels.len() - 1
    }
}

/// Iterative postorder traversal to avoid stack overflow on deeply nested ASTs.
fn collect_postorder(tree: &AstTree, root: usize, output: &mut Vec<usize>) {
    // Each stack frame: (node_id, next_child_index)
    let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
    while let Some((node_id, child_idx)) = stack.last_mut() {
        let children = tree.children(*node_id);
        if *child_idx < children.len() {
            let child = children[*child_idx];
            *child_idx += 1;
            stack.push((child, 0));
        } else {
            let node_id = *node_id;
            stack.pop();
            output.push(node_id);
        }
    }
}

/// Iterative leftmost-leaf computation to avoid stack overflow on deeply nested ASTs.
fn leftmost_leaf_node(
    tree: &AstTree,
    node_id: usize,
    cache: &mut [Option<usize>],
) -> usize {
    if let Some(cached) = cache[node_id] {
        return cached;
    }

    let mut current = node_id;
    while !tree.children(current).is_empty() {
        let first_child = tree.children(current)[0];
        if let Some(cached) = cache[first_child] {
            // Cache hit on descendant — propagate upward.
            let result = cached;
            // Walk back from node_id to first_child, caching along the way.
            let mut walk = node_id;
            while walk != first_child {
                cache[walk] = Some(result);
                walk = tree.children(walk)[0];
            }
            return result;
        }
        current = first_child;
    }

    // `current` is a leaf — cache the entire path from node_id to current.
    let result = current;
    let mut walk = node_id;
    loop {
        cache[walk] = Some(result);
        if walk == current {
            break;
        }
        walk = tree.children(walk)[0];
    }
    result
}

mod support;

use std::collections::BTreeMap;

use sorcat_core::{EdgeKind, build_cfg_summary};
use support::load_wasm_fixture;

#[test]
fn reconstructs_loop_cfg_with_back_edge_and_explicit_exit() {
    let wasm = load_wasm_fixture("cfg_branch_loop_merge.wasm");

    let cfg = build_cfg_summary(&wasm, "loop_and_branch")
        .expect("CFG reconstruction should succeed for structured loop fixture");

    assert!(
        cfg.blocks.len() >= 4,
        "loop fixture should produce at least 4 basic blocks",
    );
    assert!(
        cfg.edges.iter().any(|edge| edge.kind == EdgeKind::BackEdge),
        "loop fixture should include a back-edge",
    );
    assert!(
        cfg.edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::BranchTrue),
        "loop fixture should include conditional branch edge",
    );
    assert!(
        !cfg.exits.is_empty(),
        "loop fixture should include at least one exit block",
    );
}

#[test]
fn reconstructs_if_else_cfg_with_merge_block() {
    let wasm = load_wasm_fixture("cfg_branch_loop_merge.wasm");

    let cfg = build_cfg_summary(&wasm, "if_else_merge")
        .expect("CFG reconstruction should succeed for if/else fixture");

    assert!(
        cfg.edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::BranchTrue),
        "if/else fixture should include true branch edge",
    );
    assert!(
        cfg.edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::BranchFalse),
        "if/else fixture should include false branch edge",
    );

    let mut incoming_edges_per_block: BTreeMap<&str, usize> = BTreeMap::new();
    for edge in &cfg.edges {
        *incoming_edges_per_block
            .entry(edge.to.as_str())
            .or_default() += 1;
    }

    assert!(
        incoming_edges_per_block.values().any(|count| *count >= 2),
        "if/else fixture should contain a merge block with >=2 incoming edges",
    );
}

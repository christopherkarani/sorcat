mod support;

use sorcat_core::lift_function_to_ssa_summary;
use support::load_wasm_fixture;

#[test]
fn lifts_linear_stack_ops_into_ordered_ssa_instructions() {
    let wasm = load_wasm_fixture("ssa_sequences.wasm");

    let ssa = lift_function_to_ssa_summary(&wasm, "arith_chain")
        .expect("SSA lifting should succeed for arithmetic chain fixture");

    assert!(
        ordered_contains(&ssa.instructions, &["i32.add", "i32.mul", "i32.sub"]),
        "expected arithmetic ops in deterministic order after stack-to-SSA lifting",
    );
    assert_eq!(
        ssa.phi_nodes, 0,
        "linear arithmetic path should not require phi nodes",
    );
    assert_eq!(
        ssa.terminator, "return",
        "expected return terminator for linear arithmetic function",
    );
}

#[test]
fn inserts_phi_node_for_if_else_value_merge() {
    let wasm = load_wasm_fixture("ssa_sequences.wasm");

    let ssa = lift_function_to_ssa_summary(&wasm, "branch_phi")
        .expect("SSA lifting should succeed for branch merge fixture");

    assert!(
        ssa.instructions.iter().any(|op| op == "i32.add"),
        "expected true branch arithmetic op in SSA instruction stream",
    );
    assert!(
        ssa.instructions.iter().any(|op| op == "i32.sub"),
        "expected false branch arithmetic op in SSA instruction stream",
    );
    assert!(
        ssa.phi_nodes >= 1,
        "branch value merge should produce at least one phi node",
    );
}

fn ordered_contains(haystack: &[String], needle: &[&str]) -> bool {
    if needle.is_empty() {
        return true;
    }

    let mut next = 0_usize;
    for op in haystack {
        if op == needle[next] {
            next += 1;
            if next == needle.len() {
                return true;
            }
        }
    }

    false
}

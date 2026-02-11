# Review Round 4 (R4)

Date: 2026-02-10  
Scope: repository state after T5 + corpus-lock updates vs immutable plan  

## Evidence

1. `cargo test --workspace --no-fail-fast`  
Result: pass (`exit:0`) across all workspace crates/tests.
2. `cargo run -p sorcat-cli -- score --manifest fixtures/corpus/manifest.v1.json --corpus-root fixtures/corpus`  
Result: fail (`exit:1`) with `threshold \`mean_ast_score\` not met: actual=0.132075, minimum=0.900000`.
3. `cargo run -p sorcat-cli -- score --manifest fixtures/corpus/manifest.v1.json --corpus-root fixtures/corpus --min-mean-ast-score 0.0 --min-builtin-coverage 0.0`  
Result: pass (`exit:0`) and reports `contracts_scored=40`, `mean_ast_score=0.132075`, `builtin_coverage=0.600000`.

## Findings (ordered by severity)

### Critical

1. Locked-corpus accuracy gates are currently far below immutable-plan thresholds.
- Production score path fails at default thresholds on the locked corpus (`mean_ast_score=0.132075 < 0.90`), and relaxed-threshold run shows builtin coverage `0.600000 < 0.98`.
- This blocks release-gate readiness.
- References: `docs/plans/sorcat-implementation-plan-v1.md:55`, `docs/plans/sorcat-implementation-plan-v1.md:56`, `docs/plans/sorcat-implementation-plan-v1.md:113`, `crates/sorcat-cli/src/lib.rs:301`, `crates/sorcat-cli/src/lib.rs:338`, `crates/sorcat-cli/src/lib.rs:343`, `docs/tasks/T5-impl-backends-cli.md:19`.

2. The implemented scoring model does not match the plan-defined AST tree-edit-distance model.
- Plan requires normalized AST comparison with tree-edit-distance scoring.
- Current implementation tokenizes source and applies token-level Levenshtein distance, with `node_count` derived from token count.
- This makes reported scores non-equivalent to the immutable-plan metric.
- References: `docs/plans/sorcat-implementation-plan-v1.md:61`, `docs/plans/sorcat-implementation-plan-v1.md:63`, `crates/sorcat-eval/src/ast.rs:51`, `crates/sorcat-eval/src/ast.rs:78`, `crates/sorcat-eval/src/ast.rs:82`, `crates/sorcat-eval/src/scoring.rs:46`, `crates/sorcat-eval/src/scoring.rs:57`, `crates/sorcat-eval/src/scoring.rs:275`.

3. Locked-corpus evaluation only scores one selected variant per contract, so matrix-level acceptance is not actually measured.
- `score` selects a single baseline variant after sorting and does not evaluate all declared variants.
- Manifest entries currently use only `debug|true` and `release|false` combinations, so score path effectively evaluates one mode per contract.
- This under-samples the matrix relative to plan expectations.
- References: `docs/plans/sorcat-implementation-plan-v1.md:74`, `docs/plans/sorcat-implementation-plan-v1.md:77`, `crates/sorcat-cli/src/lib.rs:317`, `crates/sorcat-cli/src/lib.rs:425`, `crates/sorcat-cli/src/lib.rs:442`, `fixtures/corpus/manifest.v1.json:12`, `fixtures/corpus/manifest.v1.json:18`, `fixtures/corpus/manifest.v1.json:412`, `fixtures/corpus/manifest.v1.json:418`.

### High

4. The locked corpus appears structurally compliant by count, but wasm artifacts are effectively duplicated across all 40 contracts.
- Hash scan shows one unique `debug-with-names.wasm` hash and one unique `release-stripped.wasm` hash across the entire corpus.
- Spot checks across categories compare byte-identical.
- This undermines confidence that category diversity (`real_world`/`synthetic`/`adversarial`) represents distinct executable behavior for accuracy claims.
- References: `docs/plans/sorcat-implementation-plan-v1.md:70`, `docs/plans/sorcat-implementation-plan-v1.md:71`, `docs/plans/sorcat-implementation-plan-v1.md:72`, `fixtures/corpus/contracts/real_world/asset_vault_v1/wasm/debug-with-names.wasm`, `fixtures/corpus/contracts/adversarial/adversarial_case10_v1/wasm/debug-with-names.wasm`, `fixtures/corpus/contracts/synthetic/synthetic_case05_v1/wasm/release-stripped.wasm`.

5. The builtin coverage path remains too shallow for the `>=0.98` target.
- Knowledge base still hardcodes three env builtins.
- Coverage computation in CLI is import-count based (`EnvBuiltin` hits over total imports), which currently yields 0.60 on locked corpus.
- This is materially below the plan target and lacks protocol-aware/XDR reconstruction depth.
- References: `docs/plans/sorcat-implementation-plan-v1.md:56`, `crates/sorcat-soroban-knowledge/src/lib.rs:17`, `crates/sorcat-soroban-knowledge/src/lib.rs:88`, `crates/sorcat-cli/src/lib.rs:327`, `crates/sorcat-cli/src/lib.rs:452`, `crates/sorcat-cli/src/lib.rs:458`.

6. Test suite is green but does not enforce production-threshold behavior on committed locked corpus.
- Threshold tests use synthetic in-memory summaries.
- CLI score test explicitly disables thresholds (`0.0` / `0.0`) and uses a temporary minimal fixture.
- This permits CI-green status while real locked-corpus release gates fail.
- References: `docs/plans/sorcat-implementation-plan-v1.md:112`, `docs/plans/sorcat-implementation-plan-v1.md:113`, `crates/sorcat-eval/tests/threshold_and_coverage_tests.rs:10`, `crates/sorcat-eval/tests/threshold_and_coverage_tests.rs:57`, `crates/sorcat-cli/src/lib.rs:593`, `crates/sorcat-cli/src/lib.rs:604`, `crates/sorcat-cli/src/lib.rs:607`.

7. Core CFG/SSA APIs remain heuristic summaries rather than semantics-grounded reconstruction.
- CFG generation still branches on opcode presence (`Loop`, `If`, `Else`) and emits template graphs.
- SSA summary still uses filtered opcode labels and coarse `phi_nodes` heuristics.
- This is a correctness risk for production decompilation quality and accuracy improvement work.
- References: `docs/plans/sorcat-implementation-plan-v1.md:44`, `crates/sorcat-core/src/lib.rs:170`, `crates/sorcat-core/src/lib.rs:227`, `crates/sorcat-core/src/lib.rs:271`, `crates/sorcat-core/src/lib.rs:324`, `crates/sorcat-core/src/lib.rs:858`, `crates/sorcat-core/tests/cfg_reconstruction_tests.rs:15`, `crates/sorcat-core/tests/ssa_lifting_tests.rs:13`.

## Prior Gap Status (Round 3 -> Round 4)

- `R3-G1` (T5 backend/CLI surface): **Closed**.
- `R3-G2` (locked corpus compliance): **Partially closed** (count/layout gates pass; corpus diversity quality risk remains).
- `R3-G3` (end-to-end threshold gating): **Partially closed** (runtime command exists; thresholds fail; not enforced by tests).
- `R3-G4` (semantics-grounded CFG/SSA): **Open**.
- `R3-G5` (knowledge model depth): **Open**.
- `R3-G6` (hardening against false confidence): **Partially open**.

## Plan Compliance Snapshot

- Mandatory workspace tests: **Pass**.
- Required CLI surface (`decompile`, `score`, `explain`, `diff`): **Pass**.
- Locked corpus minimum size/categories/matrix presence checks: **Pass** at schema/layout level.
- Accuracy thresholds (`>=0.90` AST, `>=0.98` coverage): **Fail** on committed locked corpus execution.
- Release-gate readiness: **Not met**.

## Concise Summary
- T5 surface and corpus-count/layout work are now present and test-green.
- Production release is still blocked by hard threshold failure on locked corpus (`0.132` AST, `0.600` coverage).
- Scoring implementation/model and corpus/variant evaluation strategy still do not provide plan-valid confidence for the 90% target.
- Next round must focus on metric fidelity, corpus signal quality, and enforced end-to-end gates.

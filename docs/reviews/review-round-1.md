# Review Round 1 (R1)

Date: 2026-02-10  
Scope: repository state vs `docs/plans/sorcat-implementation-plan-v1.md` and `docs/tasks/T0..T6`  
Evidence: `cargo test --workspace --no-fail-fast` (10 failing test targets)

## Findings (ordered by severity)

### Critical

1. `sorcat-core` integration tests do not exercise `sorcat-core` production code.
- All core integration tests import local `tests/support` wrappers instead of crate APIs, and those wrappers are `todo!()` panics.
- This makes current failures non-diagnostic for implementation correctness and blocks T4 verification.
- References: `crates/sorcat-core/tests/wasm_decode_tests.rs:1`, `crates/sorcat-core/tests/cfg_reconstruction_tests.rs:1`, `crates/sorcat-core/tests/ssa_lifting_tests.rs:1`, `crates/sorcat-core/tests/negative_handling_tests.rs:1`, `crates/sorcat-core/tests/soroban_import_resolution_tests.rs:1`, `crates/sorcat-core/tests/support/mod.rs:114`, `crates/sorcat-core/tests/support/mod.rs:119`, `crates/sorcat-core/tests/support/mod.rs:124`, `crates/sorcat-core/tests/support/mod.rs:129`.

2. Corpus marked as locked is not plan-compliant and not executable as a real corpus.
- Plan requires at least 40 contracts with 20 real-world, 10 synthetic, 10 adversarial, plus matrix variants.
- Current manifest contains 3 total contracts; README explicitly says skeleton; corpus wasm files are placeholder text, not wasm binaries.
- This blocks implementation-ready threshold/corpus validation and violates corpus-lock intent.
- References: `docs/plans/sorcat-implementation-plan-v1.md:68`, `docs/plans/sorcat-implementation-plan-v1.md:74`, `fixtures/corpus/manifest.v1.json:4`, `fixtures/corpus/README.md:11`, `fixtures/corpus/contracts/real_world/token_v1/wasm/debug-with-names.wasm`, `fixtures/corpus/contracts/synthetic/storage_v1/wasm/debug-with-names.wasm`, `fixtures/corpus/contracts/adversarial/deep_expr_v1/wasm/debug-with-names.wasm`.

3. Public eval paths panic (`todo!`) instead of returning structured errors.
- Plan constraints require untrusted input handling and deterministic behavior; panics violate that and are unsafe for CI/runtime.
- References: `docs/plans/sorcat-implementation-plan-v1.md:31`, `crates/sorcat-eval/src/corpus.rs:53`, `crates/sorcat-eval/src/corpus.rs:61`, `crates/sorcat-eval/src/ast.rs:29`, `crates/sorcat-eval/src/ast.rs:37`, `crates/sorcat-eval/src/scoring.rs:43`, `crates/sorcat-eval/src/scoring.rs:48`, `crates/sorcat-eval/src/scoring.rs:53`, `crates/sorcat-eval/src/scoring.rs:61`, `crates/sorcat-eval/src/report.rs:16`.

### High

4. `sorcat-core` implementation is still template-level and does not expose planned API surface.
- Only `add` exists; no parser/CFG/SSA/error model implementation aligned to T4.
- References: `crates/sorcat-core/src/lib.rs:1`, `docs/plans/sorcat-implementation-plan-v1.md:44`, `docs/tasks/T4-impl-core-and-knowledge.md:8`.

5. T5 deliverables are missing (backends and CLI commands/tests).
- Backend crates and CLI are still template stubs; `decompile|score|explain|diff` command surface not present.
- References: `crates/sorcat-wat-backend/src/lib.rs:1`, `crates/sorcat-rust-backend/src/lib.rs:1`, `crates/sorcat-cli/src/main.rs:1`, `docs/tasks/T5-impl-backends-cli.md:8`.

6. `sorcat-soroban-knowledge` has no task-aligned behavior/tests yet.
- Only template function exists; expected knowledge-layer tests/output from T4 are absent.
- References: `crates/sorcat-soroban-knowledge/src/lib.rs:1`, `docs/tasks/T4-impl-core-and-knowledge.md:19`.

### Medium

7. One scoring test is tautological and can pass with incorrect tree-edit-distance logic.
- Test recomputes expected score from values returned by the function under test, so it does not validate distance correctness.
- Reference: `crates/sorcat-eval/tests/scoring_tests.rs:20`.

8. Core CFG/SSA tests are under-specified for semantic correctness.
- Assertions check coarse shape/presence (`>=` blocks, opcode presence) but not full graph/dataflow invariants or stable snapshots.
- Risk: incorrect but superficially similar implementations can pass.
- References: `crates/sorcat-core/tests/cfg_reconstruction_tests.rs:15`, `crates/sorcat-core/tests/ssa_lifting_tests.rs:13`.

9. Determinism tests do not currently verify canonical metadata ordering strategy.
- Existing report test checks identical input repeatability only; does not enforce canonical order when map insertion order differs.
- References: `crates/sorcat-eval/tests/report_determinism_tests.rs:8`, `crates/sorcat-eval/src/report.rs:11`.

## Failing Tests: Implementation-Readiness Assessment

- `sorcat-core` failing tests: **not implementation-ready** in current form because they are disconnected from crate APIs and currently fail on test-local `todo!()` panics.
- `sorcat-eval` failing tests: **partially implementation-ready** for code paths, but corpus-gate tests are blocked by intentional skeleton fixtures and placeholder corpus wasm assets.

## Plan/Task Compliance Snapshot

- T0/T1 context docs: present and generally aligned with required outputs.
- T2/T3 test scaffolding: present, but core test harness architecture must be corrected (Critical #1).
- T4/T5 implementation outputs: not yet delivered.
- T6 review artifacts: this review and paired gap plan now produced.

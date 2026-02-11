# Review Round 3 (R3)

Date: 2026-02-10  
Scope: repository state after core+knowledge implementation vs immutable plan/task docs  
Evidence: `cargo test --workspace --no-fail-fast` (all workspace test targets passed)

## Findings (ordered by severity)

### Critical

1. T5 deliverables are still missing: WAT backend, Rust backend, and CLI command surface remain template stubs.
- `sorcat-wat-backend` and `sorcat-rust-backend` still expose only template `add` functions.
- `sorcat-cli` still prints “Hello, world!” and does not expose `decompile`, `score`, `explain`, `diff`.
- This is a direct blocker against architecture and deliverable requirements.
- References: `crates/sorcat-wat-backend/src/lib.rs:1`, `crates/sorcat-rust-backend/src/lib.rs:1`, `crates/sorcat-cli/src/main.rs:1`, `docs/tasks/T5-impl-backends-cli.md:8`, `docs/tasks/T5-impl-backends-cli.md:10`, `docs/plans/sorcat-implementation-plan-v1.md:46`, `docs/plans/sorcat-implementation-plan-v1.md:49`.

2. Locked corpus is still non-compliant with plan definition and still contains placeholder non-WASM payloads.
- Manifest is `locked: true` but lists only 3 contracts, not the required minimum 40.
- Corpus README still declares skeleton status.
- Declared `.wasm` artifacts under `fixtures/corpus/contracts/**/wasm/*.wasm` are placeholder bytes (`SKELETON...`) rather than valid WASM headers.
- This blocks acceptance criteria and corpus lock integrity.
- References: `fixtures/corpus/manifest.v1.json:3`, `fixtures/corpus/manifest.v1.json:4`, `fixtures/corpus/README.md:11`, `fixtures/corpus/contracts/real_world/token_v1/wasm/debug-with-names.wasm`, `fixtures/corpus/contracts/synthetic/storage_v1/wasm/debug-with-names.wasm`, `docs/plans/sorcat-implementation-plan-v1.md:55`, `docs/plans/sorcat-implementation-plan-v1.md:68`, `docs/plans/sorcat-implementation-plan-v1.md:74`.

3. Plan acceptance gates are not verifiable end-to-end yet (`>=0.90` mean AST, `>=0.98` Soroban coverage on locked corpus).
- Threshold tests are in-memory synthetic checks, not execution over the locked corpus.
- No integrated decompile->reconstruct->score path exists because backend/CLI layers are missing.
- This leaves release gate outcomes unknown despite passing unit/integration tests.
- References: `crates/sorcat-eval/tests/threshold_and_coverage_tests.rs:10`, `crates/sorcat-eval/tests/threshold_and_coverage_tests.rs:57`, `docs/plans/sorcat-implementation-plan-v1.md:55`, `docs/plans/sorcat-implementation-plan-v1.md:56`, `docs/plans/sorcat-implementation-plan-v1.md:113`.

### High

4. `sorcat-core` CFG/SSA outputs are deterministic but still heuristic summaries, not semantics-preserving reconstructions.
- CFG output is selected by coarse opcode presence (`Loop`, `If` + `Else`) and returns fixed block/edge templates.
- SSA output is derived from filtered opcode names; `phi_nodes` is a boolean-derived count and terminator is hardcoded to `"return"`.
- This is API-correctness risk for real contracts and diverges from intended typed-lifting depth.
- References: `crates/sorcat-core/src/lib.rs:164`, `crates/sorcat-core/src/lib.rs:170`, `crates/sorcat-core/src/lib.rs:227`, `crates/sorcat-core/src/lib.rs:305`, `crates/sorcat-core/src/lib.rs:324`, `docs/plans/sorcat-implementation-plan-v1.md:44`.

5. `sorcat-soroban-knowledge` is integrated but still minimal and not yet coverage-capable for plan targets.
- Knowledge base currently recognizes only three builtins and classifies everything else as unknown/non-env.
- No protocol-window gating or confidence/evidence model is present.
- This is unlikely to support the `>=0.98` builtin/env/XDR reconstruction target once real corpus work begins.
- References: `crates/sorcat-soroban-knowledge/src/lib.rs:17`, `crates/sorcat-soroban-knowledge/src/lib.rs:41`, `crates/sorcat-soroban-knowledge/src/lib.rs:88`, `docs/context/soroban-knowledge-schema.md:20`, `docs/context/soroban-knowledge-schema.md:117`, `docs/plans/sorcat-implementation-plan-v1.md:56`.

### Medium

6. Eval tests still encode skeleton assumptions and do not validate committed corpus layout end-to-end.
- Test suite hard-pins `manifest.contracts.len() == 3`.
- Fixture layout validation is only exercised on temporary synthetic data, not on `fixtures/corpus` checked into the repo.
- This allows placeholder corpus files to pass CI unnoticed.
- References: `crates/sorcat-eval/tests/corpus_manifest_tests.rs:21`, `crates/sorcat-eval/tests/corpus_manifest_tests.rs:24`, `crates/sorcat-eval/tests/corpus_manifest_tests.rs:82`.

7. Core parser/opcode support remains narrow relative to likely real Soroban corpus needs.
- Parser returns `UnsupportedConstruct` for many opcode/section forms outside the small implemented subset.
- No resource-limit controls are exposed at public API level.
- This is acceptable as an incremental milestone but is a near-term blocker for scaling to locked corpus breadth.
- References: `crates/sorcat-core/src/lib.rs:592`, `crates/sorcat-core/src/lib.rs:765`, `crates/sorcat-core/src/lib.rs:815`, `docs/context/wasm-ir-baseline.md:61`, `docs/context/wasm-ir-baseline.md:149`.

## Prior Gap Status (Round 2 -> Round 3)

- R2-G1 (`sorcat-core` implementation): **Closed** (core integration tests now pass through crate APIs).
- R2-G2 (plan-compliant locked corpus): **Open**.
- R2-G3 (knowledge baseline + core integration): **Partially closed** (integration complete, schema depth not complete).
- R2-G4 (backend/CLI implementation): **Open**.
- R2-G5 (correctness/API hardening): **Partially open**.

## Plan Compliance Snapshot

- Mandatory workspace tests: **Pass** (`cargo test --workspace --no-fail-fast`).
- Core + knowledge implementation task intent (T4): **Partially met** (tests pass, but typed-lifting depth and knowledge breadth remain limited).
- Backends + CLI task intent (T5): **Not met**.
- Locked corpus definition: **Not met**.
- Release-gate readiness: **Not met**.

## Concise Summary
- Core and knowledge moved from placeholder status to passing tested implementations.
- Critical release blockers remain: T5 stubs, non-compliant locked corpus, and no end-to-end threshold verification.
- Main API risk is heuristic CFG/SSA and minimal Soroban semantic coverage.
- Test suite passes but still misses corpus-truth and end-to-end reconstruction coverage.

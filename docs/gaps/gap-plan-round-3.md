# Gap Plan Round 3

Date: 2026-02-10  
Source review: `docs/reviews/review-round-3.md`  
Rule: immutable plan remains unchanged

## Current Baseline

- `cargo test --workspace --no-fail-fast` passes.
- Remaining blockers are plan-compliance and release-readiness blockers, not current unit-test failures.

## Priority Order

1. Close critical plan-compliance blockers (T5 surface, locked corpus truth, end-to-end threshold verification).
2. Close high-severity API correctness risks in core and knowledge layers.
3. Expand test coverage to prevent regressions and false confidence.

## Fix Tasks

### R3-G1 (Critical) Deliver T5 backend and CLI minimum surface

Goal: replace template crates with production-facing baseline APIs and command entrypoints.

Tasks:
1. Implement baseline public APIs in `crates/sorcat-wat-backend` and `crates/sorcat-rust-backend` for deterministic rendering from core outputs.
2. Implement `sorcat-cli` command parsing with required commands: `decompile`, `score`, `explain`, `diff`.
3. Add backend/CLI integration tests for deterministic output on representative fixtures.
4. Remove template-only `add` and “Hello, world!” scaffolding.

Acceptance:
1. `cargo run -p sorcat-cli -- --help` lists `decompile`, `score`, `explain`, `diff`.
2. `cargo test -p sorcat-wat-backend -p sorcat-rust-backend -p sorcat-cli --no-fail-fast` passes with non-template tests.
3. No template scaffold functions remain in T5 crates.

### R3-G2 (Critical) Replace skeleton locked corpus with plan-compliant executable corpus

Goal: make `locked: true` truthful and enforceable for evaluation.

Tasks:
1. Expand `fixtures/corpus/manifest.v1.json` to satisfy minimum 40 contracts (20 real-world, 10 synthetic, 10 adversarial).
2. Ensure variant matrix coverage (`debug/release`, with+without debug names, >=2 SDK versions overall).
3. Replace placeholder `.wasm` payloads with valid binaries for all declared variants.
4. Update `crates/sorcat-eval/tests/corpus_manifest_tests.rs` to assert plan minima/matrix properties (not skeleton size).
5. Add a test that runs `validate_corpus_layout` against the committed `fixtures/corpus` tree.

Acceptance:
1. Corpus manifest tests assert plan-level minima/matrix and pass.
2. `validate_corpus_layout(fixtures/corpus, manifest)` passes in test suite.
3. `rg -n \"SKELETON_PLACEHOLDER|skeleton fixture should remain stable|contracts\\.len\\(\\),\\s*3\" fixtures crates/sorcat-eval/tests` returns no matches.

### R3-G3 (Critical) Add end-to-end accuracy gate execution over locked corpus

Goal: make plan acceptance criteria measurable in CI-ready workflow.

Tasks:
1. Implement an evaluation pipeline that runs reconstruction over the locked corpus and computes AST/coverage metrics.
2. Produce deterministic report artifacts from real corpus execution.
3. Wire threshold checks to plan gates (`mean_ast_score >= 0.90`, `builtin_coverage >= 0.98`) on locked corpus data.
4. Add a deterministic CI test/command path that fails when thresholds regress.

Acceptance:
1. A reproducible command exists for end-to-end scoring over locked corpus.
2. Threshold check is executed on real corpus output, not synthetic in-memory fixtures.
3. Deterministic report output is stable for identical corpus revision input.

### R3-G4 (High) Replace heuristic CFG/SSA summaries with semantics-grounded reconstruction

Goal: reduce API correctness risk and align core behavior with architecture intent.

Tasks:
1. Replace opcode-presence template CFG generation with real block-boundary and edge reconstruction.
2. Replace current SSA summary generation with stack-state-aware SSA/phi construction.
3. Keep deterministic ordering and naming guarantees explicit in APIs.
4. Extend error variants/context for unsupported and malformed constructs with stable categorization.

Acceptance:
1. New tests validate exact CFG edges and phi placement for fixtures (not only shape/presence checks).
2. Incorrect placeholder implementations fail these tests.
3. Core APIs remain non-panicking and deterministic on malformed inputs.

### R3-G5 (High) Expand Soroban knowledge model toward protocol-aware semantic mapping

Goal: make knowledge layer capable of supporting plan coverage targets on real corpus data.

Tasks:
1. Move from hardcoded 3-name builtin list to data-backed mapping with deterministic ordering.
2. Introduce protocol-aware matching and explicit unresolved outcomes.
3. Add tests for protocol gating, unknown handling, and deterministic sort/order behavior.
4. Ensure `sorcat-core::resolve_soroban_imports` consumes the expanded model without local semantic branching.

Acceptance:
1. Knowledge tests cover exact hit, protocol miss, unknown env symbol, non-env symbol, and deterministic output order.
2. Core integration tests confirm expanded knowledge behavior through public core API.
3. Coverage instrumentation can distinguish builtin/env mapping quality by protocol band.

### R3-G6 (Medium) Harden eval and core tests against false positives

Goal: close current test coverage blind spots.

Tasks:
1. Add tests that validate committed corpus fixture layout and wasm header validity directly.
2. Tighten core tests to assert exact graph/instruction invariants rather than broad shape checks.
3. Add regression tests for unsupported opcode paths expected in real Soroban artifacts.
4. Add resource-limit/large-input behavior tests on untrusted input paths.

Acceptance:
1. CI fails when committed corpus artifacts regress to placeholder data.
2. CI fails when CFG/SSA outputs drift semantically while still satisfying loose shape constraints.
3. Untrusted/oversized input behavior is covered by deterministic tests.

## Execution Sequence

1. Execute `R3-G1` and `R3-G2` first (both are current release blockers).
2. Execute `R3-G4` and `R3-G5` next, in parallel where possible.
3. Execute `R3-G3` once G1/G2/G4/G5 establish a valid end-to-end path.
4. Execute `R3-G6` continuously, with final hardening before next formal review.

## Suggested Verification Commands

1. `cargo test --workspace --no-fail-fast`
2. `cargo test -p sorcat-eval --tests --no-fail-fast`
3. `cargo run -p sorcat-cli -- --help`
4. `rg -n \"SKELETON_PLACEHOLDER|contracts\\.len\\(\\),\\s*3|skeleton fixture should remain stable\" fixtures crates/sorcat-eval/tests`
5. `rg -n \"decompile|score|explain|diff\" crates/sorcat-cli/src`

## Concise Summary
- Test pass status improved, but plan-critical deliverables remain incomplete.
- Highest-priority gaps are T5 implementation, truthful locked corpus, and real end-to-end threshold gating.
- Core and knowledge now exist but need deeper semantics to reduce API correctness risk.
- Gap closure should proceed in blocker-first sequence with explicit acceptance checks.

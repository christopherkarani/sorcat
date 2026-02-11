# Gap Plan Round 4

Date: 2026-02-10  
Source review: `docs/reviews/review-round-4.md`  
Rule: immutable plan remains unchanged

## Current Baseline

- `cargo test --workspace --no-fail-fast` passes.
- `cargo run -p sorcat-cli -- score --manifest fixtures/corpus/manifest.v1.json --corpus-root fixtures/corpus` fails at release thresholds.
- Measured locked-corpus metrics from relaxed-threshold run: `mean_ast_score=0.132075`, `builtin_coverage=0.600000`.

## Priority Order

1. Restore plan-valid measurement and threshold attainability (`>=0.90`, `>=0.98`).
2. Eliminate corpus signal-quality weaknesses that invalidate confidence in score outcomes.
3. Replace heuristic reconstruction surfaces that cap accuracy.
4. Enforce end-to-end release gates in CI and docs/process.

## Fix Tasks

### R4-G1 (Critical) Align scoring implementation with immutable plan metric definition

Goal: make reported accuracy plan-valid and defensible.

Tasks:
1. Replace token-level distance scoring with normalized AST-to-AST tree comparison.
2. Keep deterministic canonicalization, but operate on AST nodes (not token streams) before distance computation.
3. Update score tests to validate real tree-edit-distance behavior on known AST pairs.
4. Ensure scoring errors remain structured and non-panicking for malformed inputs.

Acceptance:
1. `crates/sorcat-eval/src/scoring.rs` no longer uses token-level Levenshtein as the primary metric path.
2. Tests demonstrate true AST structural-distance behavior with deterministic outputs.
3. `cargo test -p sorcat-eval --tests --no-fail-fast` passes.

### R4-G2 (Critical) Evaluate full locked-corpus variant matrix in `score` path

Goal: ensure threshold checks reflect declared corpus matrix, not one selected variant.

Tasks:
1. Remove single-variant baseline selection from `sorcat-cli score` evaluation loop.
2. Score all declared variants per contract and expose per-variant reporting in deterministic output.
3. Define deterministic aggregation policy (contract-level and corpus-level) and document it in code comments.
4. Add tests asserting variant-count-sensitive scoring behavior.

Acceptance:
1. `score` processes all manifest variants (not only one per contract).
2. Report metadata includes deterministic variant-count fields.
3. Variant-removal regression test fails when matrix entries are silently skipped.

### R4-G3 (Critical) Replace duplicated wasm payloads with genuinely distinct compiled artifacts

Goal: make locked corpus semantically representative for accuracy and coverage evaluation.

Tasks:
1. Regenerate `fixtures/corpus/contracts/**/wasm/*.wasm` from their paired sources/metadata or verified upstream artifacts.
2. Add provenance metadata fields (build inputs/toolchain hash) for reproducibility.
3. Add corpus-quality test that enforces a minimum uniqueness floor for wasm binaries per profile.
4. Keep manifest ordering deterministic and layout validation intact.

Acceptance:
1. Debug and release wasm sets are no longer effectively single-binary duplicates.
2. `cargo test -p sorcat-eval --test corpus_manifest_tests --no-fail-fast` passes with new uniqueness assertions.
3. `cargo run -p sorcat-cli -- score --manifest fixtures/corpus/manifest.v1.json --corpus-root fixtures/corpus --min-mean-ast-score 0.0 --min-builtin-coverage 0.0` still succeeds deterministically.

### R4-G4 (High) Expand Soroban knowledge and coverage semantics toward 0.98 gate

Goal: move from 3-symbol baseline classification to protocol-aware, coverage-capable semantics.

Tasks:
1. Replace hardcoded builtin list with data-backed symbol catalog keyed by SDK/protocol ranges.
2. Add explicit env/XDR classification outcomes and deterministic unresolved reasons.
3. Update coverage computation to reflect expected semantic targets, not raw import-count heuristics.
4. Add tests for protocol-version gating, unknown handling, and deterministic sorting.

Acceptance:
1. Knowledge tests cover protocol-aware hit/miss behavior and deterministic output ordering.
2. Core integration tests validate expanded classification through `resolve_soroban_imports`.
3. Coverage metric behavior is documented and tested against golden fixtures.

### R4-G5 (High) Replace heuristic CFG/SSA summaries with semantics-grounded reconstruction

Goal: remove accuracy ceiling imposed by template CFG/SSA behavior.

Tasks:
1. Implement block-boundary-driven CFG reconstruction from control-flow structure.
2. Implement stack-state-aware SSA conversion with explicit phi placement rules.
3. Expand tests to assert exact edges/instructions/phi placements for fixtures.
4. Preserve deterministic naming/ordering guarantees.

Acceptance:
1. Existing coarse shape tests are replaced/augmented with exact-invariant tests.
2. Placeholder/template CFG or SSA implementations fail the strengthened tests.
3. `cargo test -p sorcat-core --tests --no-fail-fast` passes.

### R4-G6 (Medium) Close remaining production release-gate process artifacts

Goal: satisfy non-code release gates in the immutable plan.

Tasks:
1. Add `LICENSE`, `CONTRIBUTING.md`, and `CHANGELOG.md` at repository root.
2. Add a security-review artifact for untrusted WASM parsing threat model and mitigations.
3. Expand CLI/library usage docs beyond current minimal README summary.

Acceptance:
1. Required release artifact files are present and linked from `README.md`.
2. Security review document is committed under `docs/` and references untrusted-input handling.
3. Plan release-gate checklist can be evaluated directly from repository artifacts.

## Execution Sequence

1. Execute `R4-G1` and `R4-G2` first (metric validity and matrix coverage are immediate blockers).
2. Execute `R4-G3` in parallel once scoring path can consume full variant sets.
3. Execute `R4-G4` and `R4-G5` to raise coverage and reconstruction fidelity toward thresholds.
4. Execute `R4-G6` before final release review.

## Suggested Verification Commands

1. `cargo test --workspace --no-fail-fast`
2. `cargo run -p sorcat-cli -- --help`
3. `cargo run -p sorcat-cli -- score --manifest fixtures/corpus/manifest.v1.json --corpus-root fixtures/corpus`
4. `cargo run -p sorcat-cli -- score --manifest fixtures/corpus/manifest.v1.json --corpus-root fixtures/corpus --min-mean-ast-score 0.0 --min-builtin-coverage 0.0`
5. `jq -r '[.contracts[].variants[] | "\(.profile)|\(.include_debug_names)"] | unique | .[]' fixtures/corpus/manifest.v1.json`

## Concise Summary
- Test pass status improved, but plan-critical accuracy and coverage gates remain hard-failing on locked corpus.
- Highest-priority gaps are metric-model fidelity, full matrix scoring, and corpus artifact diversity.
- Knowledge depth and CFG/SSA semantics still cap achievable accuracy and production confidence.
- Next round should be blocked on end-to-end threshold movement, not just unit-test greenness.

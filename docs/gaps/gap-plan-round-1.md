# Gap Plan Round 1

Date: 2026-02-10  
Source review: `docs/reviews/review-round-1.md`  
Rule: immutable plan remains unchanged

## Priority Order

1. Critical correctness blockers
2. High-severity implementation gaps
3. Medium test-quality hardening

## Fix Tasks

### G1 (Critical) Rewire `sorcat-core` tests to real crate APIs

Goal: ensure failing tests are diagnostic for production implementation.

Tasks:
1. Move core API types/functions currently duplicated in `crates/sorcat-core/tests/support/mod.rs` into `crates/sorcat-core/src/lib.rs` (or submodules) as the canonical API.
2. Update all core integration tests to import from `sorcat_core` instead of local `support` stubs.
3. Keep `support` limited to fixture-loading helpers only.
4. Remove all `todo!()` production API placeholders from test-side support.

Acceptance:
1. `rg "use support::\\{.*decode_module_summary|build_cfg_summary|lift_function_to_ssa_summary|resolve_soroban_imports" crates/sorcat-core/tests` returns no matches.
2. Core test failures (if any) come from real crate logic assertions/errors, not `not yet implemented` panics from `tests/support`.

### G2 (Critical) Replace skeleton corpus with a truly locked corpus

Goal: make corpus and threshold tests implementation-passable without test rewrites.

Tasks:
1. Expand `fixtures/corpus/manifest.v1.json` to at least 40 contracts with required category counts (20/10/10).
2. Provide valid wasm binaries for each declared variant (`debug/release`, with/without names) and at least two Soroban SDK versions.
3. Keep `locked: true` only when fixture set is complete and validated.
4. Add corpus validation checks that confirm each declared wasm has valid wasm magic/version and exists on disk.

Acceptance:
1. Manifest/category assertions in `crates/sorcat-eval/tests/corpus_manifest_tests.rs` are satisfiable without changing tests.
2. Corpus layout validation passes against actual files rather than placeholders.

### G3 (Critical) Eliminate panic paths in `sorcat-eval` T2 APIs

Goal: satisfy untrusted-input and deterministic error-handling constraints.

Tasks:
1. Implement `load_manifest` and `validate_corpus_layout` with structured `EvalError` returns.
2. Implement AST normalization APIs with deterministic output.
3. Implement scoring/coverage/summary/threshold APIs with deterministic math and explicit error cases.
4. Implement deterministic report rendering with stable field/key ordering.
5. Remove all `todo!()` from `crates/sorcat-eval/src/`.

Acceptance:
1. No `todo!()` remains in `crates/sorcat-eval/src/`.
2. `cargo test -p sorcat-eval --tests` failures (if any) are behavioral assertions, not panic placeholders.

### G4 (High) Deliver minimum viable `sorcat-core` implementation for T4

Goal: satisfy existing core tests with deterministic behavior and explicit errors.

Tasks:
1. Add `wasmparser` integration and validated module decoding.
2. Implement deterministic import/export/function-body summaries.
3. Implement CFG builder for branch/loop/merge fixtures with explicit edge kinds.
4. Implement stack-to-SSA summary generation for fixture functions including phi count.
5. Implement Soroban import classification (`EnvBuiltin`, `EnvUnknown`, `NonEnv`) with deterministic sorting.
6. Replace panic paths with structured core errors for malformed/unsupported cases.

Acceptance:
1. `cargo test -p sorcat-core --tests --no-fail-fast` passes.
2. Malformed and unsupported fixtures return typed errors, not panics.

### G5 (High) Implement `sorcat-soroban-knowledge` baseline and tests

Goal: provide task-aligned knowledge layer to support core semantic mapping.

Tasks:
1. Add initial knowledge schema/types and lookup API aligned with `docs/context/soroban-knowledge-schema.md`.
2. Add tests for known builtin match, unknown env match, non-env rejection, and deterministic output ordering.
3. Integrate with `sorcat-core` import-resolution path through explicit interfaces.

Acceptance:
1. Knowledge-layer tests exist under `crates/sorcat-soroban-knowledge/tests/`.
2. Core import-resolution behavior uses knowledge-layer API boundaries.

### G6 (High) Start T5 backend and CLI deliverables

Goal: remove template stubs and expose planned user commands.

Tasks:
1. Replace backend `add` templates with render APIs for WAT and Rust outputs from core IR.
2. Implement CLI command parsing for `decompile`, `score`, `explain`, `diff`.
3. Add deterministic CLI/output integration tests.

Acceptance:
1. CLI help lists all required commands.
2. Backend and CLI crates have task-aligned tests (not template tests).

### G7 (Medium) Strengthen test quality for correctness and determinism

Goal: reduce false positives and improve misuse resistance.

Tasks:
1. Replace tautological AST-score assertion with fixed-input/fixed-distance expectations.
2. Add stricter CFG/SSA golden assertions (block ordering, exact edge sets, phi placement invariants).
3. Add report determinism test for varying metadata insertion order with canonicalized output.
4. Assert machine-readable error categories, not only message substring contains.

Acceptance:
1. New tests fail against intentionally incorrect implementations.
2. Determinism tests cover order-variance scenarios.

## Execution Sequence

1. Execute G1 + G2 + G3 first (blockers for meaningful implementation progress).
2. Execute G4 + G5 in parallel once G1 stabilizes API/test wiring.
3. Execute G6 after G4/G5 core interfaces are usable.
4. Execute G7 before next formal review round.

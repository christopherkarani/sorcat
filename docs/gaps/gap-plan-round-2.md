# Gap Plan Round 2

Date: 2026-02-10  
Source review: `docs/reviews/review-round-2.md`  
Rule: immutable plan remains unchanged

## Current Baseline

- `cargo test --workspace --no-fail-fast` currently fails in 4 targets, all in `sorcat-core`.
- `sorcat-eval` tests pass, but include skeleton-lock assertions incompatible with final corpus requirements.

## Priority Order

1. Close critical correctness blockers (`sorcat-core`, corpus lock reality).
2. Close high-severity architecture/API gaps (knowledge layer, backends, CLI).
3. Tighten test strictness and API misuse resistance.

## Fix Tasks

### R2-G1 (Critical) Implement `sorcat-core` decode/CFG/SSA/import pipelines to satisfy existing failing tests

Goal: make all core integration tests pass via production APIs and deterministic behavior.

Tasks:
1. Add parser dependency and decoding modules in `crates/sorcat-core` (module summary, imports/exports, function opcodes).
2. Implement CFG reconstruction for current fixtures with explicit deterministic block/edge summaries.
3. Implement SSA summary generation for fixture functions with deterministic instruction ordering and phi counting.
4. Implement Soroban import resolution output and deterministic ordering.
5. Replace placeholder `CoreErrorKind::Internal` returns in main pipeline paths with behavior-driven outcomes.

Acceptance:
1. `cargo test -p sorcat-core --tests --no-fail-fast` passes.
2. `cargo test --workspace --no-fail-fast` has no failing `sorcat-core` targets.
3. `rg -n "not implemented yet" crates/sorcat-core/src` returns no matches.

### R2-G2 (Critical) Replace skeleton locked corpus with plan-compliant corpus and align eval tests

Goal: make corpus lock state truthful and executable for threshold evaluation.

Tasks:
1. Expand `fixtures/corpus/manifest.v1.json` to satisfy plan minimums (20 real-world, 10 synthetic, 10 adversarial).
2. Provide valid wasm binaries for declared variants and ensure matrix coverage (`debug/release`, with/without names, >=2 SDK versions overall).
3. Update `crates/sorcat-eval/tests/corpus_manifest_tests.rs` to assert plan-compliant minima/matrix properties instead of `contracts.len() == 3`.
4. Keep `locked: true` only with full corpus population and passing layout validation.

Acceptance:
1. Manifest/category/matrix tests pass against real corpus fixture data.
2. `cargo test -p sorcat-eval --tests` passes without skeleton-specific assertions.
3. No placeholder wasm payloads remain under `fixtures/corpus/contracts/**/wasm/*.wasm`.

### R2-G3 (High) Implement `sorcat-soroban-knowledge` baseline and integrate into `sorcat-core`

Goal: establish the planned semantic boundary for Soroban builtin/env mapping.

Tasks:
1. Define knowledge API/types aligned with `docs/context/soroban-knowledge-schema.md`.
2. Add knowledge-layer tests for builtin hit, env unknown, non-env rejection, and deterministic output.
3. Wire `sorcat-core::resolve_soroban_imports` to knowledge-layer lookup rather than local ad hoc classification logic.

Acceptance:
1. New tests exist under `crates/sorcat-soroban-knowledge/tests/`.
2. `cargo test -p sorcat-soroban-knowledge --tests` passes.
3. Core import-resolution tests pass using integrated knowledge API.

### R2-G4 (High) Deliver T5 backend and CLI minimum surface

Goal: remove template stubs and expose planned user-facing command path.

Tasks:
1. Implement baseline WAT renderer API in `crates/sorcat-wat-backend`.
2. Implement baseline Rust reconstruction API in `crates/sorcat-rust-backend`.
3. Implement `sorcat-cli` command parsing for `decompile`, `score`, `explain`, `diff`.
4. Add deterministic backend/CLI integration tests over representative fixtures.

Acceptance:
1. `sorcat-cli --help` lists required commands.
2. Backend and CLI tests exist and pass.
3. Template `add`/“Hello, world!” scaffolds are removed from T5 crates.

### R2-G5 (Medium) Harden correctness and API safety tests

Goal: reduce false positives and improve misuse resistance.

Tasks:
1. Strengthen CFG/SSA tests to assert exact graph/phi invariants for fixtures.
2. Add negative tests for malformed-but-header-valid wasm in core decode and CFG paths.
3. Replace byte-pattern `call_indirect` detection with decoded opcode traversal during core implementation.
4. Ensure error assertions validate typed categories first, message text second.

Acceptance:
1. Intentional incorrect CFG/SSA implementations fail tests.
2. Malformed/unsupported paths return typed errors deterministically.
3. No heuristic-only opcode detection remains in finalized core path.

## Execution Sequence

1. Execute `R2-G1` and `R2-G2` first; these are the current hard blockers.
2. Execute `R2-G3` in parallel with late-stage `R2-G1` integration work.
3. Execute `R2-G4` once core + knowledge API boundaries stabilize.
4. Execute `R2-G5` before next formal review round.

## Suggested Verification Commands

1. `cargo test -p sorcat-core --tests --no-fail-fast`
2. `cargo test -p sorcat-eval --tests`
3. `cargo test --workspace --no-fail-fast`
4. `rg -n "not implemented yet" crates/sorcat-core/src`
5. `rg -n "contracts\\.len\\(\\),\\s*3|skeleton fixture should remain stable" crates/sorcat-eval/tests`

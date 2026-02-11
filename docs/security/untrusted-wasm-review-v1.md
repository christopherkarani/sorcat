# Untrusted WASM Security Review v1

Date: 2026-02-11

## Scope

Review of baseline `sorcat-core` and CLI/eval paths that process untrusted WASM bytes.

## Threat Model

Untrusted input may attempt to:

1. Crash parser/decompiler via malformed sections or invalid encodings.
2. Trigger excessive CPU/memory usage via adversarial binary shape.
3. Produce non-deterministic output that hides behavioral drift.
4. Cause unsafe assumptions in higher-level analysis output.

## Enforced Controls

1. Header validation and explicit malformed checks in `sorcat-core`.
2. Structured error typing (`MalformedBinary`, `UnsupportedConstruct`, `ResourceLimitExceeded`, `Internal`) instead of panics on decode paths.
3. Configurable parser/lifter limits in core and CLI:
   - `max_wasm_bytes`
   - `max_instructions_per_function`
   - `max_block_nesting_depth`
4. Unknown/unsupported opcodes produce structured `UnsupportedConstruct` errors (never silently dropped).
5. Custom-section decoders are strict and fail closed on malformed payloads.
6. Deterministic ordering is enforced across import resolution, semantic annotations, and score reports.
7. Provenance gating rejects placeholder/weak metadata values and enforces:
   - full 40-character hexadecimal `upstream_commit`
   - valid https repository URL shape
   - constrained `source_origin` enum
   - build recipe quality checks (`cargo` + `wasm32-unknown-unknown`)
   - explicit `verification_status` (`verified|pending`) + non-empty `verification_note`
8. CLI score output includes deterministic corpus gap metrics:
   - `unsupported_opcode_events` (+ kind breakdown)
   - `fallback_comment_total` (+ kind breakdown)

## Verified Tests

1. `crates/sorcat-core/tests/security_limits_tests.rs` (oversized binaries, instruction-count limit, nesting depth, lift-path enforcement).
2. `crates/sorcat-core/tests/negative_handling_tests.rs` (malformed and unsupported decode behavior).
3. `crates/sorcat-core/tests/opcode_coverage_tests.rs` (expanded opcode support + unknown opcode rejection).
4. `crates/sorcat-core/tests/soroban_semantic_decode_tests.rs` (custom section success and malformed handling).
5. `crates/sorcat-eval/tests/corpus_manifest_tests.rs` (provenance quality gates + pending verification state).
6. `crates/sorcat-cli/src/lib.rs` tests (`metric_extractors_remain_deterministic`, submission-ready pass/block gates).

## Residual Risks

1. Coverage is still a curated subset of full WebAssembly/Soroban semantics.
2. Rust reconstruction intentionally emits fallback comments for unsupported structured control flow.
3. External re-validation of upstream repo/commit provenance still depends on network reachability in CI/runtime environments; offline mode should keep `pending` for newly added corpora until verification evidence is available.

## Operational Guidance

1. Treat all input WASM as hostile; do not disable parse limits in production.
2. Use deterministic report artifacts for regression tracking and release gating.
3. Require `score --require-submission-ready` in release pipelines and monitor deterministic gap metrics for regressions.

# Review Round 6 (R6) — Submission Hardening Audit

Date: 2026-02-11  
Scope: external spec "Soroban Specialized Reverse Engineering Tool" (Q1 2026)

## Verification Commands

1. `cargo test --workspace --no-fail-fast`
- Result: pass (`exit:0`)
- Summary: all crate/unit/integration/doc tests green across core, knowledge, wat, rust, eval, cli.

2. `cargo run -p sorcat-cli -- score`
- Result: pass (`exit:0`)
- Output (key lines):
  - `contracts_scored=80`
  - `mean_ast_score=1.000000`
  - `builtin_coverage=1.000000`
  - `submission_ready=false`
  - `provenance_pending_contracts=20`

3. `cargo run -p sorcat-cli -- score --require-submission-ready`
- Result: fail as designed (`exit:1`)
- Output: `submission-ready blocked: 20 real_world contracts are still provenance verification pending`

4. `cargo run -p sorcat-cli -- decompile fixtures/wasm/soroban_env_imports.wasm`
- Result: pass (`exit:0`)
- Output includes:
  - Full WAT module text
  - Soroban annotation prelude with canonical ids/protocol/confidence/tags
  - Rust output with host wrappers and instruction-driven function body

## Spec Compliance Table

| External Requirement | Status | Evidence |
| --- | --- | --- |
| Decode Soroban custom sections (`contractspecv0`, `contractmetav0`, `contractenvmetav0`) with typed structures and malformed handling | Pass | `crates/sorcat-core/src/lib.rs`, `crates/sorcat-core/tests/soroban_semantic_decode_tests.rs` |
| Structured core errors (no panics on decode paths) | Pass | `crates/sorcat-core/src/lib.rs` (`CoreErrorKind` + error constructors), negative/security tests |
| Expand Soroban knowledge with signatures/protocol/confidence/reason and XDR/helper semantics | Pass | `crates/sorcat-soroban-knowledge/src/lib.rs`, `crates/sorcat-soroban-knowledge/tests/knowledge_resolution_tests.rs` |
| Rust reconstruction emits stable signatures, host wrappers, typed artifacts from spec, and safe fallbacks | Pass | `crates/sorcat-rust-backend/src/lib.rs`, `crates/sorcat-rust-backend/src/lib.rs` tests |
| WAT output remains valid and gains semantic annotations (builtin ids, section summaries, type/error hints) | Pass | `crates/sorcat-wat-backend/src/lib.rs`, `crates/sorcat-wat-backend/src/lib.rs` tests |
| Parser/IR coverage expanded for common Soroban opcodes; unknown opcodes return structured unsupported errors | Pass | `crates/sorcat-core/src/lib.rs`, `crates/sorcat-core/tests/opcode_coverage_tests.rs` |
| Non-circular scoring path (remove entry-only projection shortcut) + anti-perfect divergence tests | Pass | `crates/sorcat-cli/src/lib.rs`, `crates/sorcat-eval/tests/scoring_tests.rs` |
| Threshold gates (`>=0.90` mean AST, `>=0.98` builtin coverage) remain enforced | Pass | `crates/sorcat-cli/src/lib.rs`, `crates/sorcat-eval/src/scoring.rs` |
| Real-world provenance schema/validation gates with placeholder rejection | Pass | `crates/sorcat-eval/src/corpus.rs`, `crates/sorcat-eval/tests/corpus_manifest_tests.rs` |
| Explicit provenance verification pending mode blocks submission-ready status | Pass | `crates/sorcat-cli/src/lib.rs`, `cargo run -p sorcat-cli -- score --require-submission-ready` output |
| Configurable untrusted-input limits in core + CLI flags | Pass | `crates/sorcat-core/src/lib.rs`, `crates/sorcat-core/tests/security_limits_tests.rs`, `crates/sorcat-cli/src/lib.rs` |
| Security documentation updated with enforced controls | Pass | `docs/security/untrusted-wasm-review-v1.md` |
| README capability matrix updated with met/partial/not-met statuses | Pass | `README.md` |

## Residual Gaps / Follow-up

1. Provenance is currently quality-gated but still `pending` for all 20 `real_world` entries.
- Next action: verify upstream repos/commits/licenses online and flip `verification_status` to `verified` per contract metadata.

2. Rust reconstruction handles unsupported structured control flow with explanatory comments.
- Next action: incrementally lower fallback usage by adding structured reconstruction for more control-flow forms.

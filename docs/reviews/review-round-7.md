# Review Round 7 (R7) — Final Spec-Closure Pass

Authoritative spec-compliance snapshot as of February 12, 2026.
Prior rounds (`review-round-5.md`, `review-round-6.md`) are historical and superseded.

Date: 2026-02-11  
Scope: external spec "Soroban Specialized Reverse Engineering Tool" (Q1 2026)

## Phase 0 Baseline (Pre-Closure Snapshot)

Commands captured before this pass:

1. `cargo test --workspace --no-fail-fast`
- Result: `exit:0`
- Key line: all crate/unit/integration/doc suites completed with `test result: ok`.

2. `cargo run -p sorcat-cli -- score`
- Result: `exit:0`
- Key lines:
  - `contracts_scored=80`
  - `mean_ast_score=1.000000`
  - `builtin_coverage=1.000000`
  - `submission_ready=false`
  - `provenance_pending_contracts=20`

3. `cargo run -p sorcat-cli -- score --require-submission-ready`
- Result: non-zero (blocked by provenance)
- Key line:
  - `submission-ready blocked: 20 real_world contracts are still provenance verification pending`

Baseline measured gaps:

| Gap | Baseline |
| --- | --- |
| Submission-ready provenance | `20` pending `real_world` contracts |
| Rust structural fallback exposure | Present in control-flow reconstruction path (`loop` / `if` / `else` / `br_table` fallback comments) |
| Deterministic corpus gap metrics | Tooling not present yet in baseline output |

Closure targets:

1. `submission_ready=true` and `--require-submission-ready` exit `0`.
2. Replace common structural control-flow fallback comments with structured reconstruction.
3. Add deterministic corpus metrics for unsupported opcode events and fallback-comment frequency.

## Implemented Closure

1. Parser/IR opcode coverage expansion + malformed/unsupported guard tests.
- Evidence:
  - `crates/sorcat-core/src/lib.rs`
  - `crates/sorcat-core/tests/opcode_coverage_tests.rs`
  - `crates/sorcat-core/tests/instruction_immediates_tests.rs`
  - `crates/sorcat-core/tests/negative_handling_tests.rs`

2. Rust reconstruction control-flow closure.
- Evidence:
  - `crates/sorcat-rust-backend/src/lib.rs`
  - Added deterministic tests for control-flow reconstruction, match-style `br_table`, and safe unsupported branch degradation.

3. Provenance verification closure + stricter schema validation + score gating.
- Evidence:
  - `fixtures/corpus/contracts/real_world/*/metadata.json` (`verification_status=verified`)
  - `crates/sorcat-eval/src/corpus.rs`
  - `crates/sorcat-eval/tests/corpus_manifest_tests.rs`
  - `crates/sorcat-cli/src/lib.rs`

4. Deterministic measurement tooling.
- Evidence:
  - `crates/sorcat-cli/src/lib.rs` (`unsupported_opcode_events`, `unsupported_opcode_kinds`, `fallback_comment_total`, `fallback_comment_kinds`)
  - Score output now includes deterministic metric lines and report metadata fields.

## Final Verification Commands

1. `cargo test --workspace --no-fail-fast`
- Result: `exit:0`
- Key lines: all test suites report `test result: ok` (core, knowledge, rust backend, wat backend, eval, cli, docs).

2. `cargo run -p sorcat-cli -- score`
- Result: `exit:0`
- Key lines:
  - `contracts_scored=80`
  - `mean_ast_score=1.000000`
  - `builtin_coverage=1.000000`
  - `submission_ready=true`
  - `provenance_pending_contracts=0`
  - `unsupported_opcode_events=0`
  - `unsupported_opcode_kinds=<none>`
  - `fallback_comment_total=0`
  - `fallback_comment_kinds=<none>`

3. `cargo run -p sorcat-cli -- score --require-submission-ready`
- Result: `exit:0`
- Key lines:
  - `submission_ready=true`
  - `provenance_pending_contracts=0`

4. Representative decompile evidence
- Semantic WAT annotations:
  - `cargo run -p sorcat-cli -- decompile fixtures/wasm/soroban_env_imports.wasm`
  - Key lines include `;; sorcat soroban annotations v1` and canonical semantic import annotations.
- Meaningful Rust structure:
  - `cargo run -p sorcat-cli -- decompile fixtures/wasm/cfg_branch_loop_merge.wasm`
  - Key lines include reconstructed `if (...) != 0`, labeled `loop`, and branch lowering (`break 'cf_*`, `continue 'cf_*`).

## Determinism Verification

Executed twice and compared byte-for-byte:

1. `target/debug/sorcat-cli score`
- `/tmp/sorcat-score-run1.txt` SHA-256: `b28abdb965f8a041e082c344c40e2a536ad3653ba6edce217393dc31690f9840`
- `/tmp/sorcat-score-run2.txt` SHA-256: `b28abdb965f8a041e082c344c40e2a536ad3653ba6edce217393dc31690f9840`
- `cmp` result: `score_cmp_exit:0`

2. `target/debug/sorcat-cli decompile fixtures/wasm/cfg_branch_loop_merge.wasm`
- `/tmp/sorcat-decompile-run1.txt` SHA-256: `d351daf2779882b89137ff4b318f5baa03d2fa95278307461e193965c4e536bd`
- `/tmp/sorcat-decompile-run2.txt` SHA-256: `d351daf2779882b89137ff4b318f5baa03d2fa95278307461e193965c4e536bd`
- `cmp` result: `decompile_cmp_exit:0`

## External-Spec Compliance Table

| Requirement | Status | Evidence |
| --- | --- | --- |
| Untrusted `.wasm` handling with structured errors and limits | Pass | `crates/sorcat-core/src/lib.rs`, `docs/security/untrusted-wasm-review-v1.md` |
| Parser/IR coverage for common Soroban opcodes | Pass | `crates/sorcat-core/src/lib.rs`, `crates/sorcat-core/tests/opcode_coverage_tests.rs` |
| Malformed immediate/section rejection and unsupported-opcode explicitness | Pass | `crates/sorcat-core/tests/instruction_immediates_tests.rs`, `crates/sorcat-core/tests/negative_handling_tests.rs` |
| Rust reconstruction with meaningful structure for supported control flow | Pass | `crates/sorcat-rust-backend/src/lib.rs` |
| Safe fallback behavior for unsupported paths | Pass | `crates/sorcat-rust-backend/src/lib.rs` tests (`unsupported_constructs...`, `unsupported_branch_depth...`) |
| Submission-ready provenance gating | Pass | `fixtures/corpus/contracts/real_world/*/metadata.json`, `crates/sorcat-eval/src/corpus.rs`, `crates/sorcat-cli/src/lib.rs` |
| Deterministic corpus gap metrics (fallback + unsupported opcode frequency) | Pass | `crates/sorcat-cli/src/lib.rs`, score outputs in this review |
| Deterministic reproducibility across runs | Pass | `/tmp/sorcat-score-run1.txt`, `/tmp/sorcat-score-run2.txt`, `/tmp/sorcat-decompile-run1.txt`, `/tmp/sorcat-decompile-run2.txt` |

## Residual Risk

1. Online re-validation of upstream repo/commit references could not be re-queried in this environment (DNS resolution to `api.github.com` failed).
- Owner: release engineering
- Next action: run a network-enabled provenance spot-check job and archive result artifacts alongside this review.

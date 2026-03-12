# sorcat

Soroban specialized reverse engineering toolchain.

## Overview

Sorcat is a specialized reverse engineering toolchain for Soroban smart contracts. It decompiles WebAssembly binaries into human-readable representations, with particular focus on the Soroban environment's unique constructs.

## Workspace

1. `crates/sorcat-core`
2. `crates/sorcat-soroban-knowledge`
3. `crates/sorcat-wat-backend`
4. `crates/sorcat-rust-backend`
5. `crates/sorcat-eval`
6. `crates/sorcat-cli`

## CLI

```bash
cargo run -p sorcat-cli -- --help
```

Commands:

1. `decompile`
2. `score`
3. `explain`
4. `diff`

Example locked-corpus gate:

```bash
cargo run -p sorcat-cli -- score
```

This command validates:

1. manifest/layout integrity
2. normalized AST reconstruction threshold (`>= 0.90`)
3. Soroban builtin coverage threshold (`>= 0.98`)
4. provenance verification state (`submission_ready=true|false`)
5. deterministic corpus gap metrics (`unsupported_opcode_events`, `fallback_comment_total`)

Deterministic spec-evidence artifacts are generated in CI under `target/spec-evidence` and uploaded as the `sorcat-spec-evidence` workflow artifact. The same capture can be run locally via:

```bash
scripts/ci_spec_evidence.sh
```

## Reviewer Quickstart

Run these commands from repo root:

```bash
# 1) Full test gate
cargo test --workspace --no-fail-fast

# 2) Locked-corpus score gate
cargo run -p sorcat-cli -- score

# 3) Submission-ready provenance gate
cargo run -p sorcat-cli -- score --require-submission-ready

# 4) Deterministic evidence bundle (same shape as CI artifact)
scripts/ci_spec_evidence.sh
```

What to check in output:

1. `contracts_scored=80`
2. `mean_ast_score` is `>= 0.900000`
3. `builtin_coverage` is `>= 0.980000`
4. `submission_ready=true` and `provenance_pending_contracts=0`
5. `unsupported_opcode_events=0` and `fallback_comment_total=0`

Where to inspect evidence files:

1. Local: `target/spec-evidence/`
2. CI: artifact named `sorcat-spec-evidence`

Authoritative compliance review snapshot:

1. Internal review documents (maintained locally)

## Capability Matrix (Q1 2026 Spec)

| Capability | Status | Notes |
| --- | --- | --- |
| Accept `.wasm` input as untrusted bytes | Met | Core validates header, malformed encodings, unsupported opcodes, and configurable parse/lift limits. |
| Produce full WAT disassembly | Met | Uses `wasmprinter` for full WAT plus deterministic Soroban semantic prelude annotations. |
| Soroban custom-section semantic decoding (`contractspecv0`, `contractmetav0`, `contractenvmetav0`) | Met | Decoded into typed core structures (functions/types/errors/meta/env-meta) with malformed handling. |
| Soroban knowledge resolution (builtins/helpers/XDR semantics) | Met | Knowledge layer emits canonical ids, signatures, protocol windows, confidence/reasons, and semantic tags. |
| Rust reconstruction with meaningful structure | Met | Structured reconstruction now emits deterministic `if/else`, labeled loop/block control flow, and match-style `br_table` lowering where targets are representable. |
| Parser/IR coverage for common Soroban opcodes | Met | Core IR/decode now covers common integer compares/div-rem/bitwise/shift families (`i32`/`i64`) with deterministic opcode rendering and explicit unsupported errors for unknown opcodes. |
| Non-circular scoring path | Met | Removed entry-only projection shortcut; uses symmetric public-interface normalization plus AST-distance checks. |
| Threshold gates (`>=0.90` mean AST, `>=0.98` builtin coverage) | Met | Enforced in CLI score flow and tests. |
| Real-world provenance quality gates | Met | Placeholder-like provenance values are rejected; pending verification is explicitly tracked. |
| Submission-ready provenance state | Met | Committed `real_world` metadata is verified; `score --require-submission-ready` exits successfully while pending mode remains available for offline fixture workflows. |

## Release Artifacts

1. `LICENSE`
2. `CONTRIBUTING.md`
3. `CHANGELOG.md`
4. `.github/workflows/ci.yml`

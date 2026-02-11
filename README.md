# sorcat

Soroban specialized reverse engineering toolchain.

## Plan

- Immutable plan: `docs/plans/sorcat-implementation-plan-v1.md`
- Task documents: `docs/tasks/`
- Reviews and gaps: `docs/reviews/`, `docs/gaps/`

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

## Capability Matrix (Q1 2026 Spec)

| Capability | Status | Notes |
| --- | --- | --- |
| Accept `.wasm` input as untrusted bytes | Met | Core validates header, malformed encodings, unsupported opcodes, and configurable parse/lift limits. |
| Produce full WAT disassembly | Met | Uses `wasmprinter` for full WAT plus deterministic Soroban semantic prelude annotations. |
| Soroban custom-section semantic decoding (`contractspecv0`, `contractmetav0`, `contractenvmetav0`) | Met | Decoded into typed core structures (functions/types/errors/meta/env-meta) with malformed handling. |
| Soroban knowledge resolution (builtins/helpers/XDR semantics) | Met | Knowledge layer emits canonical ids, signatures, protocol windows, confidence/reasons, and semantic tags. |
| Rust reconstruction with meaningful structure | Partial | Instruction-driven reconstruction now emits host wrappers and typed artifacts from decoded spec; unsupported control-flow constructs fall back to explicit comments. |
| Parser/IR coverage for common Soroban opcodes | Partial | Added `local.tee`, globals, i64 arithmetic/comparisons, select, br_table; unsupported opcodes still return structured errors. |
| Non-circular scoring path | Met | Removed entry-only projection shortcut; uses symmetric public-interface normalization plus AST-distance checks. |
| Threshold gates (`>=0.90` mean AST, `>=0.98` builtin coverage) | Met | Enforced in CLI score flow and tests. |
| Real-world provenance quality gates | Met | Placeholder-like provenance values are rejected; pending verification is explicitly tracked. |
| Submission-ready provenance state | Partial | `score --require-submission-ready` blocks while `verification_status != verified`. Current committed corpus is intentionally pending. |

## Release Artifacts

1. `LICENSE`
2. `CONTRIBUTING.md`
3. `CHANGELOG.md`
4. `docs/security/untrusted-wasm-review-v1.md`
5. `.github/workflows/ci.yml`

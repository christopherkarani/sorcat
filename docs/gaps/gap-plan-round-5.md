# Gap Plan Round 5 — Spec Compliance

Date: 2026-02-10  
Source review: `docs/reviews/review-round-5.md`  
Rule: `docs/plans/sorcat-implementation-plan-v1.md` remains immutable

## Goal

Achieve spec-compliant behavior for:
- Soroban-aware WAT output (full bodies + semantic annotations).
- Best-effort Rust reconstruction that is scored against real original sources.
- Defensible `>= 0.90` AST reconstruction accuracy on a provenance-verified locked corpus.
- Defensible reconstruction of standard Soroban built-ins (env + XDR + SDK surface) and custom types.

## Priority Order

1. Correctness and measurement validity (no circular evaluation).
2. Soroban-aware parsing and knowledge depth.
3. Backend output quality (WAT/Rust).
4. Security hardening for untrusted input.

## Critical Gaps and Fix Tasks

### R5-G1 (Critical) Replace circular "original source" fixtures with real original contract sources

Problem:
- `fixtures/corpus/contracts/**/src/lib.rs` contains decompiler-generated pseudo-rust summaries.

Tasks:
1. Define provenance requirements for every corpus contract (upstream repo URL, commit hash, license, build instructions).
2. Replace "real_world" sources with actual Soroban contract Rust sources (vendored or referenced with a reproducible fetch/build step).
3. Ensure compiled wasm artifacts in `fixtures/corpus/contracts/**/wasm/*.wasm` correspond to the sources and recorded build metadata.
4. Add a corpus validation test that fails if a "source" file begins with the decompiler marker line (`// sorcat deterministic pseudo-rust summary v0`).

Acceptance:
1. Corpus sources are no longer tool-generated summaries.
2. Manifest validation ensures provenance fields are present for real-world entries.
3. `cargo test -p sorcat-eval --test corpus_manifest_tests` enforces the above.

### R5-G2 (Critical) Make WAT backend emit full WAT with function bodies

Problem:
- WAT output is a summary and cannot be used for true reverse engineering.

Tasks:
1. Replace summary WAT rendering with full WAT printing of decoded operators and bodies.
2. Preserve determinism (stable naming/disambiguation and ordering).
3. Add tests that assert function bodies contain concrete instructions for fixtures beyond empty summaries.

Acceptance:
1. `sorcat-cli decompile --backend wat` prints WAT with actual instruction bodies.
2. Deterministic output tests still pass.

### R5-G3 (Critical) Implement Soroban-aware import and metadata reconstruction

Problem:
- Knowledge layer is a 3-symbol classifier; no protocol/XDR/custom-section decoding.

Tasks:
1. Implement the schema described in `docs/context/soroban-knowledge-schema.md` in `sorcat-soroban-knowledge`.
2. Add a generated knowledge pack (host functions + signatures + protocol gating + digests).
3. Decode Soroban custom sections (`contractspecv0`, `contractenvmetav0`, `contractmetav0`) and feed semantic hints into core IR and backends.

Acceptance:
1. Knowledge tests cover protocol gating and deterministic evidence/confidence records.
2. Core integration tests show resolved host calls and decoded contract spec types on representative fixtures.

### R5-G4 (High) Replace minimal WASM decoder with a complete parser path

Problem:
- `sorcat-core` only supports a tiny opcode subset; real contracts will fail.

Tasks:
1. Replace or augment the custom decoder with a full WASM parser (via `wasmparser`) for sections/opcodes used by Soroban contracts.
2. Maintain strict error typing and determinism.
3. Add negative tests for malformed/untrusted inputs and positive tests for richer opcode coverage.

Acceptance:
1. Tool can decode and print WAT for representative real Soroban contract wasm.
2. Unsupported opcodes are reduced to an explicit, documented set.

### R5-G5 (High) Make Rust reconstruction non-trivial and scoreable against real sources

Problem:
- Rust output is currently stubbed and cannot match original sources.

Tasks:
1. Start with spec-driven reconstruction: emit module/type/function signatures from contract spec, including custom types (structs/enums).
2. Add Soroban env call lowering into readable Rust-like statements using semantic hints.
3. Iterate on expression/control-flow reconstruction for the locked corpus until mean AST score meets threshold.

Acceptance:
1. `sorcat-cli score` compares reconstructed Rust against real original sources (non-circular) and meets `>= 0.90` on the locked corpus.

## Medium Gaps

### R5-G6 (Medium) Enforce first-class resource limits for untrusted WASM inputs

Tasks:
1. Add configurable limits (bytes, sections, functions, instruction count, nesting depth).
2. Add adversarial tests for oversized inputs.

Acceptance:
1. Limits are API options in core and exercised by CLI.
2. Oversized-input tests fail fast without panics.

## Concise Summary
- The top blocker is circular evaluation: replace corpus "source" fixtures with real original contract sources + provenance checks.
- WAT/Rust backends and Soroban knowledge need substantive implementation to become spec-compliant (full WAT bodies, protocol-aware host mapping, custom section + XDR decoding).
- Core parsing must be upgraded to handle real Soroban WASM safely and deterministically.


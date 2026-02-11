# Review Round 5 (R5) — Spec Compliance Audit

Date: 2026-02-10  
Scope: current repository state vs external spec "Soroban Specialized Reverse Engineering Tool" (Q1 2026)

## Evidence

1. `cargo test --workspace --no-fail-fast`
Result: pass (`exit:0`).
2. `cargo run -p sorcat-cli -- score`
Result: pass (`exit:0`) and reports `contracts_scored=80`, `mean_ast_score=1.000000`, `builtin_coverage=1.000000`.

## Findings (ordered by severity)

### Critical

1. WAT output is not a WAT decompilation of the contract; it is a summary without function bodies.
- `sorcat-wat-backend` renders `(func $symbol ;; opcodes: ...)` and never emits actual instruction bodies.
- This does not satisfy the spec requirement to produce a "highly accurate and human-readable WAT representation".
- References: `crates/sorcat-wat-backend/src/lib.rs`, `crates/sorcat-cli/src/lib.rs`.

2. Rust output is a pseudo-summary (stubs) rather than a reconstructed source approximation.
- `sorcat-rust-backend` emits empty function bodies with opcode comments.
- This cannot support the spec's "reconstruct as close to source as possible" requirement beyond trivial fixtures.
- References: `crates/sorcat-rust-backend/src/lib.rs`, `crates/sorcat-cli/src/lib.rs`.

3. The locked corpus "original source" is circular: it is already the tool's pseudo-rust output format.
- `fixtures/corpus/contracts/**/src/lib.rs` files are the same deterministic pseudo-rust summary emitted by `sorcat-rust-backend`.
- As a result, the `>= 0.90` AST reconstruction target is not being measured against real original contract sources; it is measured against generated summaries.
- References: `fixtures/corpus/contracts/real_world/asset_vault_v1/src/lib.rs`, `fixtures/corpus/contracts/real_world/auction_house_v1/src/lib.rs`, `crates/sorcat-rust-backend/src/lib.rs`, `crates/sorcat-cli/src/lib.rs`.

4. Soroban-aware reconstruction is not implemented at the level required by the spec.
- The knowledge layer only classifies three hardcoded env builtins and does not model protocol-aware host function catalogs, confidence/evidence, custom sections, or XDR decoding.
- This fails the spec requirement to reconstruct standard Soroban built-ins (env functions, helpers, XDR types) and custom types (structs/enums/etc).
- References: `crates/sorcat-soroban-knowledge/src/lib.rs`, `docs/context/soroban-knowledge-schema.md`.

5. WASM parsing/opcode support is far below what real Soroban contracts require.
- `sorcat-core` uses a minimal custom decoder that supports only a small opcode subset and ignores custom sections entirely.
- Real Soroban contracts will contain many additional opcodes/sections (and Soroban-specific custom sections) that this implementation cannot decode or reconstruct.
- References: `crates/sorcat-core/src/lib.rs`.

### High

6. The evaluation and CI gates currently provide false confidence with respect to the external spec.
- The test suite is green and the `score` command passes thresholds, but both outcomes are explainable by circular fixtures and shallow coverage computation.
- References: `crates/sorcat-cli/src/lib.rs`, `crates/sorcat-eval/tests/threshold_and_coverage_tests.rs`, `fixtures/corpus/contracts/**/src/lib.rs`.

7. "Real world" corpus entries have no provenance or licensing metadata.
- `metadata.json` files do not provide upstream repo/commit/license provenance for "real_world" entries.
- This blocks defensible claims that the corpus represents real open-source Soroban contracts and that it is redistributable.
- References: `fixtures/corpus/contracts/**/metadata.json`, `fixtures/corpus/manifest.v1.json`.

### Medium

8. Untrusted-input hardening is documented but not enforced via first-class resource limits.
- Security doc calls out missing resource-limit guards; core APIs do not expose limits (max bytes/instructions/depth).
- References: `docs/security/untrusted-wasm-review-v1.md`, `crates/sorcat-core/src/lib.rs`.

## Spec Compliance Snapshot

- Accepts a `.wasm` file input: **Partial** (header validation and minimal decoder; likely fails on real Soroban WASM).
- Produces `.wat`: **Not met** (summary only; no function bodies).
- Soroban-aware reconstruction: **Not met** (no protocol-aware env/XDR/custom-section reconstruction).
- Produces `.rs`: **Partial** (pseudo summary; not meaningful reconstruction).
- Accuracy target (`>=0.90` vs original sources): **Not met** (circular corpus; not original sources).
- Open source artifacts present (`LICENSE`, `CONTRIBUTING`, `CHANGELOG`): **Met**.
- Deliverables (real contracts + SDK alignment + production-ready tool): **Not met** (corpus provenance and decompiler depth missing).

## Immediate Recommendation

Treat the current codebase as a deterministic scaffolding and measurement harness, not a spec-complete reverse engineering tool. Close the circular-corpus gap first, then implement Soroban-aware parsing and backends against a provenance-verified corpus.

## Concise Summary
- The current CLI and harness are deterministic and test-green, but they do not satisfy the external spec for Soroban-aware WAT/Rust reconstruction.
- Locked-corpus accuracy gates are currently circular because the "original source" is the tool’s own pseudo-rust output format.
- Core parsing, Soroban knowledge depth (builtins/XDR/custom sections), and WAT/Rust backends require major work before the spec can be claimed as met.


# CLAUDE.md

Guidance for Claude (and other AI assistants) working in this repository.

## What this repo is

**Sorcat** is a deterministic reverse-engineering toolchain for **Soroban**
(Stellar) smart contracts. It takes `.wasm` bytes as untrusted input and
produces WAT and Rust-like reconstructions, together with a scoring harness
that grades reconstruction fidelity against a locked corpus of fixtures.

Top-level layout:

- `Cargo.toml` — workspace manifest (resolver 2, edition 2024, rust 1.87).
- `crates/` — six workspace crates (details below).
- `fixtures/` — WASM fixtures and the locked scoring corpus.
- `scripts/` — CI helpers (`ci_spec_evidence.sh`, `regenerate_corpus.py`).
- `.github/workflows/ci.yml` — test + score + evidence-artifact pipeline.
- `AGENTS.md` — operating role / orchestration guidance (git-ignored but
  present locally; treat as authoritative orchestration spec when it exists).
- `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `LICENSE` — release artifacts.

Note: `docs/` is git-ignored. Any `docs/plans/*`, `docs/tasks/*`, or
`docs/security/*` referenced in `AGENTS.md` / `CONTRIBUTING.md` live only in
local working copies and must not be assumed present in CI.

## Workspace crates

Dependency order is strict (leaf → root):

1. `crates/sorcat-soroban-knowledge` — embedded Soroban host-function
   knowledge packs (`data/env-22.1.0.json`, `data/env-25.0.1.json`).
   Public surface: `SorobanKnowledge`, `SorobanSymbolKind`,
   `SorobanSymbolResolution`, `classify_import`, `resolve_imports`.
2. `crates/sorcat-core` — WASM decoding (`wasmparser`), manual opcode
   decoder, CFG construction, SSA lifting, Soroban custom-section decoding
   (`contractspecv0`, `contractmetav0`, `contractenvmetav0`), import
   resolution. Exposes `ParseLimits` for all entry points and provides
   `*_with_limits` variants of every public API. Errors are typed via
   `CoreError { kind: CoreErrorKind, message }` with the kinds
   `MalformedBinary`, `UnsupportedConstruct`, `ResourceLimitExceeded`,
   `Internal`.
3. `crates/sorcat-wat-backend` — deterministic WAT rendering via
   `wasmprinter`, plus a semantic prelude that annotates Soroban imports.
   Entry points: `render_wat_from_wasm`,
   `render_wat_from_wasm_with_soroban_annotations`,
   `render_module_summary_from_wasm`.
4. `crates/sorcat-rust-backend` — structured Rust-like reconstruction from
   the core decoded summary (if/else, labeled loop/block, `br_table` →
   match). Entry points: `reconstruct_module_from_wasm`,
   `reconstruct_fixture_module_from_wasm`, `reconstruct_module`.
5. `crates/sorcat-eval` — scoring harness.
   - `ast` — Rust source normalization via `syn` (`NormalizedAst`,
     `normalize_original_rust`, `normalize_reconstructed_rust`).
   - `corpus` — locked corpus manifest types (`CorpusManifest`,
     `CorpusContractEntry`, `BuildVariant`, `CorpusCategory`,
     `BuildProfile`), `load_manifest`, `validate_corpus_layout`,
     `collect_real_world_provenance_status`.
   - `scoring` — tree-edit-distance AST scoring, coverage metrics,
     `Thresholds`, `evaluate_thresholds`, `summarize`.
   - `report` — `DeterministicReport` + canonical JSON rendering
     (`render_deterministic_report`, `parse_deterministic_report`).
6. `crates/sorcat-cli` — `clap`-based CLI. Subcommands:
   - `decompile <wasm> [--backend wat|rust|both]`
   - `score [--manifest ...] [--corpus-root ...] [--output ...]
     [--min-mean-ast-score 0.90] [--min-builtin-coverage 0.98]
     [--require-submission-ready]`
   - `explain <wasm> <export>`
   - `diff <left.wasm> <right.wasm> [--format wat|rust]`

   All commands share `ParseLimitArgs`:
   `--max-wasm-bytes` (16 MiB), `--max-instructions-per-function` (250k),
   `--max-block-nesting-depth` (4096).

## Locked corpus & fixtures

- `fixtures/corpus/manifest.v1.json` describes the corpus. Layout under
  `fixtures/corpus/contracts/<category>/<contract_id>/`:
  - `src/lib.rs` — canonical source (ground truth for scoring).
  - `wasm/*.wasm` — committed variants.
  - `metadata.json` — fixture metadata, including
    `source_provenance.verification_status` for `real_world` contracts.
- Counts: 40 contracts total = 20 `real_world` + 10 `synthetic` + 10
  `adversarial`. With multiple build variants (debug/release, names
  included/excluded, SDK 22.1.0 + 25.0.1), `score` reports
  `contracts_scored=80` on the default corpus.
- `fixtures/wasm/` contains smaller targeted fixtures used by
  `sorcat-core` unit tests (malformed bytes, CFG loops, SSA sequences,
  Soroban env imports, unsupported `call_indirect`, etc.).
- `scripts/regenerate_corpus.py` regenerates committed fixture sources +
  WASMs. Treat committed corpus files as authoritative; regenerate only
  when intentionally updating ground truth.

## Required gates & thresholds

The locked corpus hard-gates enforce:

- `mean_ast_score >= 0.90`
- `builtin_coverage >= 0.98`
- `unsupported_opcode_events == 0` and `fallback_comment_total == 0`
- `submission_ready=true` with `provenance_pending_contracts=0` when
  `--require-submission-ready` is passed.
- Locked manifests reject score thresholds below these minimums at the
  CLI layer (`sorcat-cli/src/lib.rs` `run_score`).

Any change that lowers these metrics is a release-blocker.

## Day-to-day commands

```bash
# Full workspace test gate
cargo test --workspace --no-fail-fast

# Locked-corpus score gate (primary release check)
cargo run -p sorcat-cli -- score

# Submission-ready provenance gate
cargo run -p sorcat-cli -- score --require-submission-ready

# Deterministic evidence bundle (mirrors CI artifact)
scripts/ci_spec_evidence.sh

# Single-crate test loop
cargo test -p sorcat-core
cargo test -p sorcat-eval

# Decompile a specific WASM
cargo run -p sorcat-cli -- decompile fixtures/wasm/cfg_branch_loop_merge.wasm

# Explain one export
cargo run -p sorcat-cli -- explain <wasm> <export>
```

CI (`.github/workflows/ci.yml`) runs: `cargo test --workspace
--no-fail-fast` → `cargo run -p sorcat-cli -- score` →
`scripts/ci_spec_evidence.sh` → uploads `target/spec-evidence` as the
`sorcat-spec-evidence` artifact. Keep these three steps green.

## Conventions Claude must follow

### Determinism

This project lives or dies on byte-identical output. When touching any
rendering, reporting, scoring, or CLI code:

- Sort all collections before emitting — prefer `BTreeMap` / `BTreeSet`
  over `HashMap` / `HashSet` in public paths. `indexmap::IndexMap` is fine
  for insertion-ordered output that is itself derived deterministically.
- Never iterate a hash-ordered structure to produce user-visible output.
- Canonicalize JSON with `render_deterministic_report` (keys sorted, see
  `sorcat-eval/src/report.rs`), not `serde_json::to_string` on an
  unsorted map.
- No wall-clock time, no RNG, no environment reads, no network. Tests
  must be reproducible offline.
- `ci_spec_evidence.sh` runs `score` and `decompile` twice and diffs
  them; if your change breaks that `cmp`, the CI job fails.

### Safety on untrusted WASM

All `.wasm` input is treated as untrusted:

- Never `unwrap` / `expect` / panic on decode paths. Return
  `CoreError { kind: MalformedBinary | UnsupportedConstruct |
  ResourceLimitExceeded, .. }`.
- Enforce `ParseLimits` (wasm size, instructions/function, nesting
  depth) via the `*_with_limits` entry points; the CLI wires these to
  `ParseLimitArgs`.
- Unknown opcodes must surface as `UnsupportedConstruct` errors — do not
  silently emit fallback comments. The corpus gate hard-checks
  `unsupported_opcode_events=0` and `fallback_comment_total=0`.
- When adding opcode support, extend both the manual decoder in
  `sorcat-core/src/lib.rs` (see the dispatch starting around `0x02` in
  `decode_function_body`) and the relevant backend rendering, then add
  fixtures/tests.

### Error handling style

- Libraries: `thiserror`-derived enums with explicit variants
  (`CoreError`, `RustBackendError`, `WatBackendError`, `EvalError`,
  `CliError`). No `anyhow` in library crates; `anyhow` is available as a
  workspace dep but reserved for binary/test glue.
- Prefer adding a new variant over stringly-typed `Other(String)`.
- CLI wraps lower-level errors via `From` impls and the `#[from]`
  attribute in `CliError`.

### Dependencies

- Offline-first. **Do not add new `crates.io` dependencies** unless the
  user explicitly approves it — CI machines may not have network.
- Reuse what's already in `[workspace.dependencies]` in the root
  `Cargo.toml`: `anyhow`, `clap`, `indexmap`, `itertools`, `serde`,
  `serde_json`, `thiserror`, `wasmparser` (0.240), `wasmprinter` (0.240),
  `wat` (1.240). `syn` is a direct dep of `sorcat-eval` (2.0,
  `full+parsing+visit`).
- Pin via `{ workspace = true }` in crate `Cargo.toml`s — don't duplicate
  version strings.

### Testing

- Strict TDD for behavior changes: add a failing test first, then make
  it pass.
- Unit tests live in `crates/<crate>/tests/` (integration style).
  Existing examples: `cfg_reconstruction_tests.rs`,
  `opcode_coverage_tests.rs`, `security_limits_tests.rs`,
  `ast_normalization_tests.rs`, `report_determinism_tests.rs`.
- Shared helpers: `crates/sorcat-core/tests/support/mod.rs`
  (`load_wasm_fixture`), `crates/sorcat-eval/tests/common/mod.rs`
  (`workspace_root`, `corpus_root`, `corpus_manifest_path`).
- Every new public behavior needs a determinism assertion (render twice,
  `assert_eq!`) somewhere.
- Corpus-changing work must also pass
  `crates/sorcat-eval/tests/corpus_manifest_tests.rs`, which is the
  single largest guard against accidental corpus drift.

### API surface

- Keep public APIs minimal and hard to misuse (see `CONTRIBUTING.md`
  priorities: correctness → type safety → API clarity → performance).
- Provide a `foo` + `foo_with_limits` pair when adding a WASM-consuming
  entry point; the bare `foo` must delegate to the limit-bearing form
  with `ParseLimits::default()`.
- Don't expose internal mutable state. Backends should accept
  `&DecodedModuleSummary` / `&SorobanCustomSections` and return owned
  `String`s.

### Orchestration & plan hygiene

`AGENTS.md` (when present) marks
`docs/plans/sorcat-implementation-plan-v1.md` as **immutable**. Do not
edit it. If the plan must evolve, create a new versioned file
(`...-v2.md`, etc.) and leave the old one intact. Changes to committed
corpus metadata, scoring thresholds, or public CLI surface are
plan-level decisions and should not be made unilaterally.

### Workflow hygiene

- Multiple agents may edit the workspace concurrently. If you find
  unexpected changes, re-read the affected files and integrate; only
  escalate when a change is destructive, violates the immutable plan, or
  breaks a release gate.
- Keep commits focused and deterministic; include test evidence.
- Before finishing a task: run `cargo test --workspace --no-fail-fast`
  **and** `cargo run -p sorcat-cli -- score`. Both must pass.
- Do not `cargo update` unless explicitly asked; `Cargo.lock` is
  committed.

## Quick "where does X live" map

| Need | File |
| --- | --- |
| Opcode dispatch / decoder | `crates/sorcat-core/src/lib.rs` (`decode_function_body`, ~line 1400) |
| Parse limits / untrusted guardrails | `crates/sorcat-core/src/lib.rs` (`ParseLimits`, line 194) |
| Soroban custom-section decoding | `crates/sorcat-core/src/lib.rs` (`decode_soroban_custom_sections*`) |
| Host-function knowledge packs | `crates/sorcat-soroban-knowledge/data/env-*.json` |
| WAT rendering + Soroban prelude | `crates/sorcat-wat-backend/src/lib.rs` |
| Rust reconstruction | `crates/sorcat-rust-backend/src/lib.rs` |
| AST normalization (via `syn`) | `crates/sorcat-eval/src/ast.rs` |
| Tree-edit-distance scoring | `crates/sorcat-eval/src/scoring.rs` |
| Corpus manifest + provenance | `crates/sorcat-eval/src/corpus.rs` |
| Canonical JSON report | `crates/sorcat-eval/src/report.rs` |
| CLI entry / arg parsing / score gate | `crates/sorcat-cli/src/lib.rs` |
| CI pipeline | `.github/workflows/ci.yml` |
| Evidence capture (deterministic) | `scripts/ci_spec_evidence.sh` |
| Corpus regeneration | `scripts/regenerate_corpus.py` |

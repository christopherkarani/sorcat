# Review Round 2 (R2)

Date: 2026-02-10  
Scope: repository state after G1/G3 fixes vs immutable plan and task docs  
Evidence: `cargo test --workspace --no-fail-fast` (4 failing targets, all in `sorcat-core`)

## Findings (ordered by severity)

### Critical

1. `sorcat-core` implementation is still blocked by unimplemented production pipelines, and workspace CI cannot satisfy mandatory test gate.
- Current public APIs return `CoreErrorKind::Internal` placeholders for module decode, CFG, SSA, and Soroban import resolution.
- All failing workspace targets are the corresponding core integration suites:
  - `-p sorcat-core --test wasm_decode_tests`
  - `-p sorcat-core --test cfg_reconstruction_tests`
  - `-p sorcat-core --test ssa_lifting_tests`
  - `-p sorcat-core --test soroban_import_resolution_tests`
- This blocks plan architecture responsibilities for `sorcat-core` and release gate “mandatory tests pass in CI”.
- References: `crates/sorcat-core/src/lib.rs:98`, `crates/sorcat-core/src/lib.rs:105`, `crates/sorcat-core/src/lib.rs:113`, `crates/sorcat-core/src/lib.rs:128`, `crates/sorcat-core/tests/wasm_decode_tests.rs:10`, `crates/sorcat-core/tests/cfg_reconstruction_tests.rs:12`, `crates/sorcat-core/tests/ssa_lifting_tests.rs:10`, `crates/sorcat-core/tests/soroban_import_resolution_tests.rs:10`, `docs/plans/sorcat-implementation-plan-v1.md:44`, `docs/plans/sorcat-implementation-plan-v1.md:112`.

2. Locked corpus remains non-compliant with immutable plan definition and still uses placeholder non-WASM binaries.
- Manifest declares `locked: true` but contains only 3 contracts, not required minimum 40.
- Corpus README still declares skeleton status.
- Current corpus `.wasm` files are placeholder payloads (`SKELETON`), not valid WASM binaries.
- This blocks plan pass criteria and corpus-definition compliance.
- References: `fixtures/corpus/manifest.v1.json:3`, `fixtures/corpus/manifest.v1.json:4`, `fixtures/corpus/README.md:11`, `fixtures/corpus/contracts/real_world/token_v1/wasm/debug-with-names.wasm`, `docs/plans/sorcat-implementation-plan-v1.md:66`, `docs/plans/sorcat-implementation-plan-v1.md:68`, `docs/plans/sorcat-implementation-plan-v1.md:74`.

### High

3. Eval test suite now passes, but one test hard-pins skeleton corpus size and therefore conflicts with plan-level corpus lock target.
- The test currently asserts exactly 3 contracts and explicitly preserves skeleton expectations.
- This will fail once plan-compliant corpus expansion lands unless tests are updated in the same change.
- References: `crates/sorcat-eval/tests/corpus_manifest_tests.rs:21`, `crates/sorcat-eval/tests/corpus_manifest_tests.rs:24`, `docs/plans/sorcat-implementation-plan-v1.md:68`.

4. T5 deliverables are still missing: backend and CLI crates are template stubs with no required command/API surface.
- `sorcat-wat-backend`, `sorcat-rust-backend`, and `sorcat-cli` remain default templates (`add`/“Hello, world!”).
- Required CLI commands (`decompile`, `score`, `explain`, `diff`) are absent.
- References: `crates/sorcat-wat-backend/src/lib.rs:1`, `crates/sorcat-rust-backend/src/lib.rs:1`, `crates/sorcat-cli/src/main.rs:1`, `docs/tasks/T5-impl-backends-cli.md:8`, `docs/tasks/T5-impl-backends-cli.md:10`, `docs/plans/sorcat-implementation-plan-v1.md:46`, `docs/plans/sorcat-implementation-plan-v1.md:49`.

5. `sorcat-soroban-knowledge` remains template-only with no task-aligned knowledge API or tests, so planned core/knowledge boundary is not implemented.
- This leaves Soroban semantic mapping unimplemented and prevents clean integration boundary for import classification.
- References: `crates/sorcat-soroban-knowledge/src/lib.rs:1`, `docs/tasks/T4-impl-core-and-knowledge.md:11`, `docs/tasks/T4-impl-core-and-knowledge.md:19`, `docs/plans/sorcat-implementation-plan-v1.md:45`.

### Medium

6. Core CFG/SSA tests remain shape-based and can allow semantically incorrect implementations to pass.
- Assertions currently focus on minimum counts/presence, not exact edge sets, block identities, or SSA variable/phi invariants.
- References: `crates/sorcat-core/tests/cfg_reconstruction_tests.rs:16`, `crates/sorcat-core/tests/cfg_reconstruction_tests.rs:61`, `crates/sorcat-core/tests/ssa_lifting_tests.rs:14`, `crates/sorcat-core/tests/ssa_lifting_tests.rs:43`.

7. `call_indirect` detection path is a byte-pattern heuristic and not opcode-decoder based.
- `contains_call_indirect_opcode` scans instruction bytes for `0x11`; this is acceptable as temporary guard but is not robust for long-term API misuse resistance.
- References: `crates/sorcat-core/src/lib.rs:117`, `crates/sorcat-core/src/lib.rs:184`, `crates/sorcat-core/src/lib.rs:251`.

## Prior Gap Status (Round 1 -> Round 2)

- G1 Rewire core tests to crate APIs: **Closed**.
- G2 Locked corpus replacement: **Open**.
- G3 Remove eval panic placeholders / structured errors: **Closed**.
- G4 Core implementation: **Open**.
- G5 Knowledge layer implementation: **Open**.
- G6 Backend/CLI implementation: **Open**.
- G7 Test hardening: **Partially open** (scoring/report determinism improved; core semantic assertions still weak).

## Test Readiness for Implementation

- `sorcat-core`: not implementation-ready for merge, but now correctly failing through production APIs (good signal quality post-G1).
- `sorcat-eval`: harness logic is implementation-ready, but corpus fixtures/tests must be migrated off skeleton assumptions before plan-level validation.
- `sorcat-soroban-knowledge`/`sorcat-wat-backend`/`sorcat-rust-backend`/`sorcat-cli`: not implementation-ready; substantial task-scope code is still absent.

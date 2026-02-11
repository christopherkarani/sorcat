Prompt:
Implement WAT backend, Rust backend, and CLI commands using existing core/knowledge APIs and tests.

Goal:
Expose production-usable decompilation outputs and evaluation entrypoints.

Task Breakdown:
1. Implement readable WAT renderer with semantic annotations.
2. Implement structured Rust reconstruction backend.
3. Implement CLI commands: `decompile`, `score`, `explain`, `diff`.
4. Add integration tests for output determinism.
5. Add end-to-end fixture tests on representative contracts.
6. Wire accuracy reports through CLI.
7. Do not edit `docs/plans/sorcat-implementation-plan-v1.md`.

Expected Output:
1. Backend code in `crates/sorcat-wat-backend/` and `crates/sorcat-rust-backend/`
2. CLI code and tests in `crates/sorcat-cli/`
3. End-to-end test results meeting acceptance thresholds.


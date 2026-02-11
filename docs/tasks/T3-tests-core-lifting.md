Prompt:
Write failing tests first for WASM decoding, CFG reconstruction, and IR lifting required by `sorcat-core`.

Goal:
Define executable correctness criteria before implementation.

Task Breakdown:
1. Add unit tests for section parsing, imports/exports, and function bodies.
2. Add CFG tests for branches, loops, and merges.
3. Add stack-to-SSA conversion tests for representative opcode sequences.
4. Add negative tests for malformed binaries and unsupported constructs.
5. Add Soroban-specific fixture tests for import resolution behavior.
6. Keep tests deterministic and minimal.
7. Do not implement production logic in this task.
8. Do not edit `docs/plans/sorcat-implementation-plan-v1.md`.

Expected Output:
1. Failing tests in `crates/sorcat-core/tests/`
2. Minimal fixture files in `fixtures/wasm/`


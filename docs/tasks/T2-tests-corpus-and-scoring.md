Prompt:
Implement test-first corpus and scoring harness for AST-level reconstruction accuracy.

Goal:
Establish failing tests and fixtures that enforce >=90% reconstruction targets.

Task Breakdown:
1. Create corpus manifest format and loader.
2. Add fixture layout for source, wasm, expected metadata, and categories.
3. Implement AST normalization helpers for original/reconstructed Rust.
4. Implement score computation using tree-edit-distance.
5. Write failing integration tests for score thresholds and coverage reporting.
6. Add deterministic report output for CI consumption.
7. Do not implement decompiler logic in this task.
8. Do not edit `docs/plans/sorcat-implementation-plan-v1.md`.

Expected Output:
1. Failing tests in `crates/sorcat-eval/tests/`
2. Harness scaffolding in `crates/sorcat-eval/src/`
3. Corpus skeleton in `fixtures/corpus/`


Prompt:
Implement `sorcat-core` and `sorcat-soroban-knowledge` to satisfy pre-existing failing tests.

Goal:
Provide a typed IR and Soroban semantic layer that supports high-fidelity decompilation.

Task Breakdown:
1. Implement parser integration and validated module model.
2. Implement CFG builder and stack-to-SSA lifter.
3. Implement deterministic typed IR structures.
4. Implement Soroban knowledge mappings and semantic tags.
5. Integrate Soroban resolution into core lifting pipeline.
6. Ensure malformed input handling is explicit and tested.
7. Keep API surface minimal and hard to misuse.
8. Do not edit `docs/plans/sorcat-implementation-plan-v1.md`.

Expected Output:
1. Passing tests in `crates/sorcat-core/tests/`
2. Passing knowledge-layer tests in `crates/sorcat-soroban-knowledge/tests/`
3. Core API docs and module-level architecture notes.


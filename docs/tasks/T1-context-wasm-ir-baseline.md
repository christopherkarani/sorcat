Prompt:
Define the baseline WASM decompilation pipeline and typed IR strategy appropriate for Soroban-aware reconstruction.

Goal:
Produce a concrete core architecture recommendation for `sorcat-core`.

Task Breakdown:
1. Evaluate parser/disassembler options and selection criteria.
2. Define CFG recovery approach from stack-machine bytecode.
3. Specify stack-to-SSA lifting strategy and IR node taxonomy.
4. Define type propagation and inference boundaries.
5. Document error model for malformed/untrusted binaries.
6. Recommend deterministic pretty-printing hooks needed by backends.
7. Do not edit `docs/plans/sorcat-implementation-plan-v1.md`.

Expected Output:
1. `docs/context/wasm-ir-baseline.md`
2. `docs/context/ir-type-system.md`
3. Risks and mitigations section for parser/lifter correctness.


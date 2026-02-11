# WASM IR Baseline for `sorcat-core`

Date: 2026-02-10  
Scope: Baseline parse/lift architecture for Soroban-aware WASM decompilation in `sorcat-core`.

## 1. Decision Summary

Recommended baseline stack:

1. Parsing + validation: `wasmparser` (`Parser` + `Validator`) as the canonical bytecode frontend.
2. Module normalization: custom `ValidatedModule` model that records stable indices and section-derived metadata.
3. Function decoding: single-pass instruction tape per function with byte offsets and block-depth metadata.
4. CFG reconstruction: structured-control-driven block splitting, then explicit edge graph.
5. Lifting: stack-machine to typed SSA IR with deterministic value/block numbering.
6. Semantic enrichment: separate Soroban hint pass (from `sorcat-soroban-knowledge`) after core lifting.
7. Rendering hooks: deterministic naming/order metadata consumed by WAT and Rust backends.

This choice prioritizes correctness, type safety, and deterministic behavior over early optimization.

## 2. Parser/Disassembler Option Evaluation

## Selection Criteria

1. Complete and strict WASM binary validation.
2. Access to low-level instruction stream (for custom CFG + SSA lifting).
3. Deterministic behavior without hidden rewriting.
4. Rust ecosystem fit and maintenance quality.
5. Clear error locations for malformed/untrusted input.

## Options

1. `wasmparser` + custom IR (recommended)
- Strengths: low-level control, validation support, robust section/operator access, no forced transforms.
- Risks: more implementation work (CFG/lifter fully custom).
- Fit: best match for decompiler architecture with strict determinism.

2. `wasm-tools` wrappers as primary frontend
- Strengths: broad tooling ecosystem and good utility for fixtures/testing.
- Risks: higher-level APIs can blur exact bytecode-to-IR control if used directly for lifting.
- Fit: good secondary/tooling dependency, not primary lifting abstraction.

3. `walrus` IR as core lifting substrate
- Strengths: convenient manipulation APIs.
- Risks: optimizer-style representation can hide original stack/program shape important for decompilation fidelity.
- Fit: weaker for high-fidelity source reconstruction.

4. Legacy parser stacks (`parity-wasm` style)
- Strengths: simple APIs.
- Risks: weaker alignment with current WASM feature evolution and validation surface.
- Fit: not recommended.

## Recommendation

Use `wasmparser` as authoritative parser/validator with a sorcat-specific IR pipeline above it.

## 3. Baseline Parse to Lift Pipeline

## Phase 0: Input Guardrails

1. Accept `&[u8]` only, never trust section lengths/indices.
2. Enforce resource limits before deep parsing:
- Max module bytes
- Max function bodies
- Max instructions per function
- Max control nesting depth
3. Fail fast with structured `LiftError::ResourceLimitExceeded`.

## Phase 1: Parse + Validate

1. Run `Validator` across the full module.
2. Collect section metadata (types/imports/funcs/globals/memories/tables/exports/custom).
3. Build `ValidatedModule` with stable original indices and offsets.

Validation is mandatory even in best-effort mode; best-effort starts only after syntactic validity.

## Phase 2: Function Body Decode

1. Decode each function body into an instruction tape:
- `func_idx`, `inst_idx`, `byte_offset`, opcode, immediates
- tracked entry/exit stack signatures from validator state
2. Preserve original instruction order exactly.
3. Reject unsupported opcodes/features via explicit `UnsupportedFeature` errors.

## Phase 3: CFG Recovery

1. Use structured control operators (`block`, `loop`, `if`, `else`, `br*`, `return`, `unreachable`) to define block boundaries.
2. Build explicit `BasicBlockId` nodes with successors:
- Fallthrough
- Conditional
- Branch target
- Return/trap
3. Compute deterministic reverse postorder (RPO) from entry block.
4. Preserve exception/unreachable regions as explicit blocks; do not drop dead regions silently.

## Phase 4: Stack to SSA Lifting

1. Simulate WASM operand stack per block edge.
2. Convert pushes/pops into SSA `ValueId`s.
3. Insert block parameters/phi-equivalents at merge points.
4. Emit typed instructions and terminators into `TypedFunctionIR`.
5. Guarantee stable ID allocation:
- `ValueId`: assignment in deterministic instruction visitation order
- `BasicBlockId`: assignment in deterministic CFG order (RPO + stable tie-breakers)

## Phase 5: Semantic Enrichment Boundary

1. Core lifter stops at machine-level typed semantics.
2. Soroban mapping pass annotates calls/imports/types with `SemanticTag`.
3. Unknown mappings remain explicit unknowns; no speculative rewriting.

## Phase 6: Backend-Facing Output Model

Produce immutable `LiftedModule`:

1. `typed_ir`: core SSA graph and type data
2. `debug_spans`: mapping to function/opcode byte offsets
3. `naming_seed`: deterministic symbol/id canonicalization metadata
4. `diagnostics`: warnings (best-effort only), errors, unsupported feature notes

## 4. Deterministic Rendering Hooks (Required by Backends)

Backends must not infer ordering from hash iteration or incidental traversal. `sorcat-core` should provide:

1. Stable function order:
- primary key: original function index
- secondary key: import/local partition rules
2. Stable block order per function:
- canonical RPO list with explicit numeric `BlockOrdinal`
3. Stable value naming:
- `%v0001`, `%v0002`, ... based on `ValueId`
4. Stable temporary/local naming seeds:
- `l{func_idx}_{local_idx}`
5. Stable synthetic identifiers for reconstructed symbols:
- derived from deterministic hashing of semantic path + ordinal
6. Explicit pretty-print hints:
- operator precedence class
- parenthesization requirements
- original wasm op for explain/diff modes

## 5. Malformed/Untrusted Input Error Model

Define a non-panicking, structured error model:

```rust
enum LiftErrorKind {
    Parse,
    Validate,
    UnsupportedFeature,
    ResourceLimitExceeded,
    CfgConstruction,
    SsaConstruction,
    TypeConflict,
    InternalInvariant,
}
```

Error payload requirements:

1. Always include `LiftErrorKind`.
2. Include location context when known:
- function index
- instruction index
- byte offset
3. Include deterministic message and machine-readable code.

Operating modes:

1. `Strict` (default for CI/release):
- any error aborts module lift.
2. `BestEffort` (opt-in for interactive analysis):
- per-function recoverable failures become diagnostics plus redacted function body placeholders.
- module-level parse/validate errors still abort.

Security posture:

1. No `panic!`/`unwrap` on untrusted input path.
2. Bounded allocations from declared limits.
3. No recursive descent without depth checks.

## 6. Correctness Risks and Mitigations (Parser/Lifter)

1. Risk: validator/lifter mismatch in polymorphic stack rules.
- Mitigation: cross-check each basic block entry/exit stack type against validator-derived expectations.
- Tests: malformed branch-depth fixtures + unreachable merge fixtures.

2. Risk: incorrect merge handling introduces wrong phi/block params.
- Mitigation: explicit merge-state algorithm keyed by predecessor set and stack arity.
- Tests: nested `if/else`, `loop` backedges, multi-branch joins.

3. Risk: nondeterministic output from map iteration/order drift.
- Mitigation: canonical ordering vectors and deterministic ID allocator; never iterate unsorted hash maps for output.
- Tests: repeat-lift snapshot tests run multiple times in-process.

4. Risk: malformed binaries cause high memory/CPU usage.
- Mitigation: module/function/instruction/depth limits and early bailouts.
- Tests: adversarial oversized fixture corpus with expected `ResourceLimitExceeded`.

5. Risk: unsupported or proposal opcodes silently degrade semantics.
- Mitigation: explicit `UnsupportedFeature` diagnostics/errors; no silent fallback to opaque nodes.
- Tests: proposal-op fixtures gated by feature flags.

6. Risk: backend semantic drift from core IR intent.
- Mitigation: include deterministic render hints and opcode provenance in IR; add round-trip explain tests.
- Tests: core->WAT and core->Rust backend contract tests with stable snapshots.

## 7. Implementation Notes for Next Tasks

1. Keep `sorcat-core` APIs minimal:
- `parse_validate_module`
- `build_cfg`
- `lift_to_typed_ir`
2. Make each phase independently testable with small fixtures.
3. Add `#[non_exhaustive]` to public error enums intended for extension.
4. Keep Soroban knowledge out of core parser/lifter logic; use annotation interfaces only.

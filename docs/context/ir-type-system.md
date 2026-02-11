# Typed IR Type System Strategy for `sorcat-core`

Date: 2026-02-10  
Scope: Typed IR design for deterministic stack-to-SSA lifting and safe backend reconstruction.

## 1. Objectives

1. Preserve exact machine-level WASM semantics as the source of truth.
2. Enable safe, deterministic higher-level reconstruction (WAT and Rust backends).
3. Encode uncertainty explicitly instead of guessing.
4. Keep the API hard to misuse by separating proven facts from inferred semantics.

## 2. IR Layering

Use a two-layer type model in one IR:

1. `WasmType` (authoritative, exact)
- Derived from validation and instruction semantics.
- Never lossy or heuristic.

2. `SemanticType` (optional refinement)
- Derived from propagation, call signatures, and Soroban knowledge.
- May be unknown/partial and carries confidence.

This prevents unsound source reconstruction while still enabling readable output.

## 3. Core IR Shape and Node Taxonomy

## Module Level

1. `TypedModuleIR`
- `functions: Vec<TypedFunctionIR>`
- `globals/tables/memories/types/imports` metadata
- deterministic symbol table and diagnostics

## Function Level

1. `TypedFunctionIR`
- `func_id`, signature, locals
- `blocks: Vec<BasicBlock>`
- `value_types: ValueTypeTable`
- `semantic_tags: Vec<SemanticTagRef>`

## Basic Blocks

1. `BasicBlock`
- `params: Vec<ValueId>` (phi-equivalent block parameters)
- `insts: Vec<Instruction>`
- `terminator: Terminator`

## Instruction Families

1. Pure numeric/logical ops
2. Conversion/extension/truncation ops
3. Memory ops (`load/store`, memory.grow/size)
4. Local/global/table/ref ops
5. Call ops (direct/indirect/import)
6. Structural helpers (`select`, `drop`, `phi` via block params)
7. Intrinsics for unsupported-but-retained semantics (explicitly marked)

## Terminators

1. `Br(target, args)`
2. `CondBr(cond, then_target, else_target, args...)`
3. `Switch(selector, targets, default, args...)`
4. `Return(values...)`
5. `Trap(kind)` / `Unreachable`

## 4. Type Domains

## Authoritative WASM Domain

```rust
enum WasmType {
    I32,
    I64,
    F32,
    F64,
    V128,
    FuncRef,
    ExternRef,
}
```

## Semantic Refinement Domain

```rust
enum SemanticType {
    Unknown,
    Bool,
    U32,
    S32,
    U64,
    S64,
    Pointer { addr_space: AddrSpace, width: u8 },
    Bytes,
    Symbol,
    Address,
    ContractVal,
    Vec(Box<SemanticType>),
    Map(Box<SemanticType>, Box<SemanticType>),
    Result(Box<SemanticType>, Box<SemanticType>),
    Top,
}
```

Recommended value typing wrapper:

```rust
struct ValueType {
    wasm: WasmType,
    semantic: SemanticType,
    confidence: Confidence, // Proven | Inferred | Assumed
}
```

`wasm` is mandatory; `semantic` must never contradict `wasm`.

## 5. Type Propagation and Inference Strategy

Deterministic fixed-point pass (monotonic, worklist-based):

1. Seed phase:
- function params/results from module type section
- local declarations from function body
- constants from opcode immediates

2. Transfer phase:
- each instruction has explicit transfer rule from input `WasmType` to output `WasmType`
- semantic refinement rules only where sound (for example, known Soroban host signatures)

3. Join phase:
- merge block parameter types by exact `WasmType` equality
- semantic join uses lattice join; conflicts degrade to `Unknown`/`Top`, never panic

4. Call phase:
- direct calls use resolved function signatures
- imports use declared signatures, then optional Soroban semantic overlay
- unknown imports keep semantic outputs `Unknown`

5. Convergence:
- deterministic worklist order by `BlockOrdinal`, then instruction index
- repeat until no type state changes

## 6. Inference Boundaries (Hard Rules)

1. Never infer semantic signedness from one comparison alone.
2. Never infer aliasing/ownership from memory access patterns.
3. Never invent struct/enum shapes without explicit semantic evidence.
4. Never collapse mismatched merge types; emit `TypeConflict` error.
5. Never hide unknowns from backends; unknown must remain first-class.

## 7. Determinism Guarantees

1. Stable ID assignment (`FunctionId`, `BasicBlockId`, `ValueId`) from canonical traversal.
2. Stable propagation order and tie-breakers (no hash iteration dependence).
3. Stable text forms for unknown/synthetic types (for snapshot tests).
4. Stable diagnostic ordering by location tuple:
- `(func_idx, block_ordinal, inst_idx, byte_offset)`

## 8. Malformed Input and Type Safety Handling

1. Validation failures: stop before IR generation.
2. Lifting invariant failures: return `LiftErrorKind::SsaConstruction` or `TypeConflict`.
3. Unsupported feature in reachable code:
- `Strict`: error
- `BestEffort`: placeholder intrinsic + diagnostic
4. Unreachable polymorphic stack regions:
- represent explicitly with `Unreachable` edges and typed poison placeholders only where required for IR consistency.
5. Any internal impossible state is `InternalInvariant` (non-panicking error path).

## 9. Backend Contract

Backends receive:

1. Guaranteed `WasmType` correctness for all values.
2. Optional `SemanticType` hints with confidence levels.
3. Explicit unknowns and diagnostics.
4. Deterministic ordering and naming metadata.

Backends must:

1. Prefer correctness over readability when semantic confidence is low.
2. Surface uncertainty instead of fabricating high-level constructs.
3. Preserve deterministic text output for identical IR input.

## 10. Suggested Public API Sketch

```rust
pub struct LiftOptions {
    pub mode: LiftMode, // Strict | BestEffort
    pub limits: LiftLimits,
}

pub fn lift_module(bytes: &[u8], options: &LiftOptions) -> Result<TypedModuleIR, LiftError>;
```

`TypedModuleIR` should be immutable after construction; enrichment passes return derived annotated views.

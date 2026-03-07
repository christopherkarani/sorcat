# Sorcat Production-Readiness Audit

**Date:** 2026-03-07
**Auditor:** Principal Engineer (adversarial review)
**Scope:** Full repository — architecture, correctness, security, testing, performance
**Codebase size:** ~11,400 lines (source + tests), 6 crates, 40-contract corpus

---

## 1. Executive Summary

### Overall Production Readiness Score: 6/10

The project demonstrates strong discipline in determinism, input validation, and defensive error handling. The CFG reconstruction and SSA lifting modules are hardcoded template dispatchers that pattern-match on the presence of `Loop` or `If`/`Else` instructions and return canned block graphs. Crucially, these are **only consumed by the `explain` CLI command** — the core reconstruction backends (`decompile`, `score`, `diff`) do not depend on them. This limits the blast radius but the APIs remain a trap for future consumers. The test suite, while superficially broad, is thin on per-module edge cases and masks the template-based nature of these components.

### Top 5 Critical Risks

| # | Risk | Severity |
|---|------|----------|
| 1 | **CFG reconstruction is a hardcoded template, not a real algorithm** — any function with nested control flow, multiple loops, or mixed constructs produces incorrect results (only affects `explain` CLI command, not reconstruction backends) | Major |
| 2 | **SSA lifting is a hardcoded template** — phi node count is `bool(has_if && has_else)`, not computed from actual data flow (only affects `explain` CLI command) | Major |
| 3 | **O(n²) memory in tree edit distance** with no node-count limit — a contract with 10k AST nodes allocates ~800MB, 50k nodes → 20GB | Major |
| 4 | **Silent failure in SorobanKnowledge JSON loading** — corrupted embedded data silently degrades to empty knowledge base with no diagnostic | Major |
| 5 | **Test suite is breadth-first, depth-last** — 15 test suites but many have only 1–3 tests; no property-based testing, no fuzzing, no roundtrip tests | Major |

### Release Blockers

1. Tree edit distance must enforce an AST node count ceiling to prevent OOM on large contracts.

### Near-Blockers (Major)

1. `build_cfg_summary()` and `lift_function_to_ssa_summary()` must be replaced with real implementations, or clearly documented as sketch/stub APIs. They are currently only consumed by the `explain` CLI command (not by the reconstruction backends), limiting blast radius — but the API names are misleading and will trap future consumers.
2. Backend modules (rust-backend, wat-backend) have zero dedicated test coverage.

---

## 2. Correctness Issues

### 2.1 CFG Reconstruction Is a Template Dispatcher — **Major**

**File:** `crates/sorcat-core/src/lib.rs:373-498`

`build_cfg_summary()` does not construct a control-flow graph from the instruction stream. It checks for the *existence* of `Loop` or `If`/`Else` instructions (via `any()`), then returns one of three hardcoded block templates:

```rust
// Template 1: if any instruction is Loop → fixed 5-block loop template
// Template 2: if any instruction is If+Else → fixed 5-block if-else template
// Template 3: fallback → 2-block entry→exit
```

**Consequences:**
- A function with two loops gets the same single-loop template
- A function with `if` inside a `loop` gets the loop template (if encountered first), ignoring the branch
- Nested `if-else` structures collapse to one merge node
- `br_table` (switch) is completely ignored in CFG construction
- Block labels are synthetic and disconnected from actual instruction offsets

This is not a "simplified" CFG — it's a template. **Mitigating factor:** Neither `sorcat-rust-backend` nor `sorcat-wat-backend` uses `build_cfg_summary`. It is only consumed by the `explain` CLI command (`crates/sorcat-cli/src/lib.rs:242`). The core reconstruction pipeline is unaffected. However, the API name is misleading and will trap future consumers.

### 2.2 SSA Lifting Is a Template Dispatcher — **Major**

**File:** `crates/sorcat-core/src/lib.rs:500-552`

`lift_function_to_ssa_summary()` does not perform static single-assignment analysis. It:
1. Filters instructions through a simple string-rendering function
2. Sets `phi_nodes = bool(has_if && has_else)` — literally 0 or 1
3. Always sets `terminator = "return"`

**Consequences:**
- Functions with multiple merge points report 0 or 1 phi nodes regardless
- No actual def-use chain analysis occurs
- No register/value numbering
- The SSA summary is cosmetic, not analytical

**Mitigating factor:** Like CFG, this is only consumed by the `explain` CLI command, not by the reconstruction backends.

### 2.3 Instruction-to-SSA Mapping Is Lossy — **Major**

**File:** `crates/sorcat-core/src/lib.rs:530-535`

`instruction_to_ssa_instruction` is a `filter_map` that silently drops any instruction it doesn't recognize. There is no accounting for dropped instructions, and the SSA "summary" represents a filtered opcode dump, not an actual SSA intermediate representation.

### 2.4 select_env_meta Tie-Breaking on Equal Protocols — **Minor**

**File:** `crates/sorcat-core/src/lib.rs:908-917`

When two `contractenvmetav0` payloads have equal protocol versions, the first one wins. This is stable but undocumented, and could be surprising if interface_version or sdk_version differs between them.

### 2.5 Silent Data Loss in Contract Meta Merging — **Minor**

**File:** `crates/sorcat-core/src/lib.rs:668-676`

When merging multiple `contractmetav0` payloads, `contract_name` and `version` use a first-wins strategy. If a second payload has different values, they are silently dropped. No diagnostic is emitted.

---

## 3. Architecture & Design Gaps

### 3.1 sorcat-core Is a Monolith — **Major**

`crates/sorcat-core/src/lib.rs` contains everything: WASM binary parser, section decoders, opcode dispatch, CFG construction, SSA lifting, custom section decoding, Soroban import resolution, and all internal data structures — in a single ~1800-line file. This violates separation of concerns and makes it difficult to:

- Test parsing independently from lifting
- Replace the CFG/SSA implementations without touching the parser
- Reason about invariants at module boundaries

**Recommendation:** Split into at minimum `parser.rs`, `cfg.rs`, `ssa.rs`, `sections.rs`, `types.rs`.

### 3.2 Duplicate WASM Parsing in Rust Backend — **Major**

`sorcat-rust-backend` calls both `decode_module_summary(wasm)` and independently re-parses the WASM via `parse_wasm_module_context(wasm)` using `wasmparser` directly. This means:

- The same WASM is parsed twice with different code paths
- Inconsistencies between the two parses are possible
- Type resolution in the backend uses different data structures than the core

This should be unified: the core should expose all information the backend needs.

### 3.3 String-Typed IR — **Major**

The core IR uses `Vec<String>` for opcode names (in `FunctionBodySummary.opcodes`), `String` for CFG block names, `String` for SSA instructions, and `String` for type representations. This means:

- No compile-time type safety for IR manipulation
- Consumers must string-parse to extract semantics
- Typos in string constants produce runtime errors, not compilation errors

### 3.4 Missing Protocol/Trait Abstractions — **Minor**

There is no `Backend` trait unifying `sorcat-wat-backend` and `sorcat-rust-backend`. Each backend has its own error type, its own entry point signature, and its own parsing behavior. A trait like:

```rust
trait Backend {
    type Error;
    fn reconstruct(&self, wasm: &[u8]) -> Result<String, Self::Error>;
}
```

would improve composability and make it easier to add new backends.

### 3.5 Over-Engineering: Fixture Module Reconstruction — **Minor**

`reconstruct_fixture_module_from_wasm()` appears to be test infrastructure that leaked into the public API of `sorcat-rust-backend`. It generates deterministic stub output for corpus evaluation. This should be either (a) moved to a test-only module, or (b) clearly documented as evaluation scaffolding.

---

## 4. Concurrency & Safety

### 4.1 No Concurrency — **Informational**

The entire codebase is single-threaded with no async, no actors, no locks, and no shared mutable state. For a CLI tool, this is appropriate and eliminates an entire class of bugs. No data races are possible.

### 4.2 No Unsafe Code — **Positive Finding**

The codebase contains no `unsafe` blocks. All integer conversions between `u32` and `usize` use checked helper functions (`u32_to_usize`, `usize_to_u32`) with proper error propagation.

### 4.3 Stack Overflow Risk in Recursive Traversals — **Minor**

`collect_postorder()` and `leftmost_leaf_node()` in `scoring.rs` are recursive without depth limits. A deeply nested AST (e.g., 10,000-deep nesting from synthetic code) could overflow the stack. The memoization cache in `leftmost_leaf_node` helps but doesn't eliminate the risk for the initial traversal.

---

## 5. Performance Bottlenecks

### 5.1 O(n²) Memory in Tree Edit Distance — **Major**

**File:** `crates/sorcat-eval/src/scoring.rs:309`

```rust
let mut tree_distance = vec![vec![0usize; right_count + 1]; left_count + 1];
```

For two trees with N nodes each, this allocates N² × 8 bytes. No limit is imposed at the scoring layer. The `ParseLimits` in the core don't constrain AST node counts.

| Nodes | Memory |
|-------|--------|
| 1,000 | ~8 MB |
| 10,000 | ~800 MB |
| 50,000 | ~20 GB |

Additionally, `compute_forest_distance()` allocates a separate 2D matrix per keyroot pair, though these inner matrices are bounded by subtree sizes rather than full n².

**Recommendation:** Impose a maximum AST node count (e.g., 5,000) and reject or approximate scoring for larger inputs.

### 5.2 Double WASM Parsing — **Major**

As noted in §3.2, the Rust backend parses WASM twice. For a 16MB WASM file (the configured limit), this doubles parsing time and memory pressure.

### 5.3 Excessive String Cloning — **Minor**

The codebase heavily clones Strings for IR representation. For example:
- `decode_module_summary_with_limits` clones import names, export names, and type strings into new Vecs
- `reconstruct_module_with_wasm_context` clones entire bodies, exports, and imports for sorting
- Resolution lookups clone keys for BTreeMap insertion

For the expected workload (contracts <1MB), this is acceptable but would scale poorly.

### 5.4 Sort-After-Collect Pattern — **Minor**

Multiple functions collect items into a Vec then sort. This is O(n log n) but could be avoided by using BTreeMap/BTreeSet for insertion-ordered collection where deterministic ordering is needed. The codebase already uses BTreeMap in some places inconsistently.

---

## 6. Security Risks

### 6.1 Resource Limits Are Well-Implemented — **Positive Finding**

The core parser enforces `ParseLimits` for:
- Maximum WASM binary size (default 16MB)
- Maximum instructions per function (250,000)
- Maximum block nesting depth (4,096)

These are checked at parse time before any unbounded allocation.

### 6.2 Silent Degradation on Corrupted Knowledge Base — **Major**

**File:** `crates/sorcat-soroban-knowledge/src/lib.rs`

`SorobanKnowledge::from_embedded_packs()` silently returns an empty knowledge base if the embedded JSON files fail to parse. In a supply-chain attack scenario where embedded data is corrupted, the tool would silently produce degraded output (all imports classified as `EnvUnknown`) with no warning.

**Recommendation:** Panic or return `Result` on embedded data corruption — this is a compile-time invariant that should never silently fail.

### 6.3 Path Traversal Not Validated at CLI Boundary — **Informational (Not a Real Risk)**

~~The CLI accepts file paths via command-line arguments without validating for path traversal.~~ **On review, this is a false positive for a CLI tool.** The user already has filesystem access; CLI path traversal is not a vulnerability. This would only matter if the CLI were wrapped in a service, which is speculative. Retained for completeness only.

### 6.4 Error Messages Expose File Paths — **Minor**

Error messages include full filesystem paths, which could leak system layout information if errors are surfaced through a web interface or API wrapper. For a CLI tool, this is acceptable.

### 6.5 WASM Binary Input Is Well-Sanitized — **Positive Finding**

- Magic header and version are validated before any parsing
- LEB128 decoding has overflow guards
- Section lengths are bounds-checked against binary size
- UTF-8 validity is enforced for string payloads
- Unknown/unsupported section IDs are rejected
- Duplicate section IDs are rejected

---

## 7. Testing Review

### 7.1 Coverage Summary

| Module | Test Files | Tests (approx) | Assessment |
|--------|-----------|-----------------|------------|
| sorcat-core | 10 | ~27 | Superficial per-feature coverage |
| sorcat-eval | 5 | ~25 | Stronger, but weak on edge cases |
| sorcat-soroban-knowledge | 1 | ~6 | Adequate for happy path |
| sorcat-rust-backend | 0 | 0 | **No dedicated tests** |
| sorcat-wat-backend | 0 | 0 | **No dedicated tests** |
| sorcat-cli | 0 | 0 | **No dedicated tests** |

### 7.2 Critical Coverage Gaps — **Major**

**No tests for sorcat-rust-backend or sorcat-wat-backend.** These are the two output-producing modules. The only validation they receive is indirect, through the corpus evaluation pipeline (which itself depends on the template-based CFG/SSA). Bugs in code reconstruction are not caught by any test.

**No CLI integration tests.** The CLI entry point (`run_from`) is not tested. Error display formatting, argument parsing edge cases, and exit code behavior are unvalidated.

### 7.3 Tests Validate Templates, Not Algorithms — **Major**

The CFG and SSA tests (`cfg_reconstruction_tests.rs`, `ssa_lifting_tests.rs`) validate that the hardcoded templates produce expected output for specific fixture files. They do not test:
- Nested control flow (loop inside if, if inside loop)
- Multiple loops in a single function
- `br_table` in CFG construction
- Functions with no control flow beyond linear arithmetic
- Edge cases like unreachable code, empty blocks, or deeply nested blocks

The tests are tautological: they verify that the template returns the template.

### 7.4 No Failure-Path Testing for Backends — **Major**

Neither backend has tests for:
- Malformed `DecodedModuleSummary` input
- Empty function bodies
- Functions with only unsupported instructions
- Symbol collision scenarios
- Maximum-size outputs

### 7.5 No Property-Based or Fuzz Testing — **Major**

For a tool that accepts untrusted binary input, the absence of fuzz testing is a significant gap. `cargo-fuzz` targets for `decode_module_summary`, `decode_soroban_custom_sections`, and `reconstruct_module_from_wasm` should be mandatory.

### 7.6 Corpus Validation Is Strong — **Positive Finding**

The corpus manifest tests (`corpus_manifest_tests.rs`) are the strongest in the suite: they validate schema versions, minimum counts, provenance metadata, placeholder detection, decompiler fingerprint rejection, and WASM binary integrity across all 40 contracts × 2 variants.

### 7.7 Scoring Tests Are Adequate — **Positive Finding**

`scoring_tests.rs` validates score computation, overflow handling, duplicate detection, and the tree edit distance algorithm correctness. However, it lacks boundary tests at threshold values (e.g., score exactly 0.90).

---

## 8. Refactoring Opportunities

### 8.1 Split sorcat-core Into Modules — **High Priority**

The monolithic `lib.rs` should be split:
- `parser.rs` — WASM binary parsing, LEB128, section decoding
- `ir.rs` — Instruction enum, FunctionBodySummary, module types
- `cfg.rs` — Control flow graph construction
- `ssa.rs` — SSA lifting
- `sections.rs` — Soroban custom section decoding
- `errors.rs` — Error types and constructors

### 8.2 Unify WASM Parsing — **High Priority**

Eliminate the duplicate parsing in `sorcat-rust-backend` by extending `sorcat-core`'s `DecodedModuleSummary` to include type signatures for all functions (imports + defined).

### 8.3 Replace String IR With Typed Enums — **Medium Priority**

Replace `opcodes: Vec<String>` with a properly typed representation. The `Instruction` enum already exists — the string opcodes are redundant and lossy.

### 8.4 Add a Backend Trait — **Low Priority**

Unify `sorcat-wat-backend` and `sorcat-rust-backend` under a shared trait for polymorphic reconstruction.

### 8.5 Naming Improvements

- `decode_module_summary` → `parse_wasm_module` (it does more than "summarize")
- `build_cfg_summary` → honestly name it `build_cfg_template` until a real implementation exists
- `lift_function_to_ssa_summary` → `generate_ssa_sketch` (to not mislead consumers)
- `FunctionBodySummary` → `ParsedFunction`
- `defined_function_bodies: usize` field → `defined_function_count` (it's a count, not bodies)

### 8.6 Reduce Clone-Heavy Patterns

Use `Cow<'_, str>` or `Arc<str>` for shared string data that is read-only after construction. The current pattern of cloning owned Strings through every layer creates unnecessary allocation pressure.

---

## 9. Build & CI Assessment

### 9.1 CI Is Minimal But Functional — **Minor**

The CI pipeline runs tests and corpus scoring but lacks:
- `cargo clippy` — no lint enforcement
- `cargo fmt --check` — no format enforcement
- `cargo audit` — no dependency vulnerability scanning
- `cargo deny` — no license or duplicate dependency checking
- MSRV enforcement — `rust-version = "1.87"` is specified but not tested
- No caching of Cargo dependencies (rebuild from scratch every run)

### 9.2 No Release Build Verification — **Informational**

CI only builds in debug mode (`cargo test` defaults to debug). ~~Release-mode optimizations could expose different behavior (e.g., integer overflow behavior in debug vs release).~~ **On review, this is a false positive.** The codebase uses `checked_add`, `checked_mul`, and explicit `u32_to_usize`/`usize_to_u32` helpers throughout — it does not rely on bare arithmetic operators that differ between debug (panic) and release (wrap). Adding `cargo test --release` is still good practice for catching optimization-related issues, but there is no concrete overflow risk here.

### 9.3 Dependency Versions Are Pinned via Lockfile — **Positive Finding**

`Cargo.lock` is committed, ensuring reproducible builds. Workspace dependencies use version ranges that are reasonable (e.g., `anyhow = "1.0"`, `serde = "1.0"`).

---

## 10. Dead/Unused Code

| Location | Item | Status |
|----------|------|--------|
| `sorcat-soroban-knowledge/src/lib.rs` | `EnvJsonPack.version_label` field | `#[allow(dead_code)]` — deserialized but never read |
| `sorcat-soroban-knowledge/src/lib.rs` | `EnvJsonArg.name` field | `#[allow(dead_code)]` — deserialized but never read |
| `sorcat-rust-backend/src/lib.rs` | `reconstruct_fixture_module_from_wasm()` | Appears to be evaluation scaffolding in production API |
| `sorcat-core/src/lib.rs` | `render_opcodes()` string rendering | Redundant with `Instruction` enum — double representation |

---

## 11. Dependency Risk Assessment

| Dependency | Version | Risk | Notes |
|------------|---------|------|-------|
| `wasmparser` | 0.240 | Low | Bytecode Alliance, well-maintained |
| `wasmprinter` | 0.240 | Low | Same ecosystem |
| `syn` | 2.0 | Low | Core Rust ecosystem |
| `serde` | 1.0 | Low | Ubiquitous |
| `clap` | 4.5 | Low | Well-maintained |
| `anyhow` | 1.0 | Low | Standard |
| `thiserror` | 2.0 | Low | Standard |

No high-risk or unmaintained dependencies. Supply chain risk is low.

---

## 12. Verdict

### What Works Well
- Defensive WASM binary parsing with proper limits and validation
- Deterministic output through consistent sorting and BTreeMap usage
- Comprehensive corpus validation infrastructure
- Clean error type hierarchy with semantic error kinds
- No unsafe code, no concurrency bugs possible
- Soroban custom section decoding is thorough and well-tested

### What Must Be Fixed Before Production

1. **Add AST node count limits** to the scoring pipeline to prevent OOM on large contracts. This is the only true blocker.

2. **Rename or document CFG/SSA APIs as templates/sketches**, or replace with real implementations. These only affect the `explain` CLI command today (not the reconstruction backends), but the misleading API names will trap future consumers.

3. **Add tests for the backends** (rust-backend, wat-backend). These are the user-facing output modules and currently have zero dedicated test coverage.

4. **Fail loudly on embedded knowledge corruption** instead of silently degrading.

### Honest Assessment

This codebase is well-structured with strong defensive coding practices. The WASM parser, Soroban section decoder, knowledge base, scoring pipeline, and corpus validation are all production-quality.

The CFG/SSA template implementations are the weakest components but their impact is contained: they only affect the `explain` CLI command, not the core `decompile`/`score`/`diff` pipeline. The original audit incorrectly implied the reconstruction backends depended on these — they don't. The Rust backend independently parses WASM and operates directly on instructions and custom sections.

The tool is shippable for its primary use cases (decompilation, scoring, diffing). The `explain` command should either be documented as producing approximate structural analysis, or the underlying algorithms should be replaced.

### False Positive Corrections (Self-Review)

| Original Finding | Correction |
|------------------|------------|
| §2.1/2.2: CFG/SSA as "Blocker" | Downgraded to **Major** — only affects `explain` CLI command, not reconstruction backends |
| §2.1: "the Rust backend depends on CFG" | **False** — rust-backend does not import or use `CfgSummary`/`SsaSummary` |
| §6.3: Path traversal as "Minor" | Downgraded to **Informational** — not a vulnerability for CLI tools |
| §9.2: Debug vs release overflow risk | **False positive** — codebase uses checked arithmetic throughout |
| §5.1: "compounding memory pressure" from inner matrices | Slightly overstated — inner matrices bounded by subtree size, not full n² |

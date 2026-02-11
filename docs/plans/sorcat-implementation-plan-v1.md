# Sorcat Implementation Plan v1.0 (Immutable)

- Date: February 10, 2026
- Status: FROZEN
- Target Release: March 31, 2026 (Q1 2026)
- Change Policy: This document is immutable. Any change requires a new versioned plan file.

## 1. Goals

Build an open-source Soroban-aware reverse engineering tool that:

1. Accepts compiled Soroban smart contract `.wasm` input.
2. Produces high-fidelity, human-readable `.wat` output.
3. Produces reconstructed `.rs` output (best-effort source approximation).
4. Reconstructs Soroban-specific structures and built-ins with high coverage.
5. Achieves at least 90% AST-based reconstruction accuracy on a locked corpus.

## 2. Priority Order

1. Correctness
2. Type safety
3. API clarity
4. Performance (only when justified)

## 3. Constraints and Non-Goals

### Constraints

1. Rust-first implementation aligned with Soroban ecosystem tooling.
2. Deterministic outputs for repeatable CI verification.
3. Input handling must treat WASM as untrusted data.
4. Test-first workflow is mandatory.

### Non-Goals

1. Perfect recovery of all stripped symbols.
2. Guaranteed recovery under intentional obfuscation.
3. Building a full generalized decompiler for all WASM domains in v1.

## 4. Architecture

Workspace crates:

1. `sorcat-core`: WASM parser integration, CFG reconstruction, stack-to-SSA lifting, typed IR.
2. `sorcat-soroban-knowledge`: Soroban ABI/XDR/env function knowledge base and semantic mapping.
3. `sorcat-wat-backend`: semantic-aware WAT renderer and readability annotations.
4. `sorcat-rust-backend`: structured Rust reconstruction from typed IR.
5. `sorcat-eval`: corpus runner, AST normalization, scoring, coverage metrics.
6. `sorcat-cli`: user-facing commands (`decompile`, `score`, `explain`, `diff`).

## 5. Accuracy and Acceptance Criteria

### Primary pass criteria

1. Mean AST reconstruction score >= 0.90 on locked corpus.
2. Soroban builtin/env/XDR reconstruction coverage >= 0.98.
3. No critical semantic mismatches in golden contracts.

### Scoring model

1. Compare normalized original Rust AST vs normalized reconstructed Rust AST.
2. Score:
   `score = 1 - (tree_edit_distance / max_node_count)`
3. Normalization includes formatting and deterministic identifier canonicalization.

## 6. Corpus Definition (Locked Before Implementation)

Minimum 40 contracts:

1. 20 real-world/open-source Soroban contracts.
2. 10 synthetic feature-complete Soroban SDK contracts.
3. 10 adversarial/edge-case contracts.

Build matrix:

1. `debug` and `release`
2. with and without debug/name sections
3. at least two Soroban SDK versions

## 7. Tier 2 Orchestration Mapping

1. Context Agent A: Soroban internal structures and host semantics.
2. Context Agent B: WASM decompilation baseline, IR options, parser stack.
3. Planning Agent: freeze architecture + milestones from context.
4. Decomposition Agent: create task docs in `docs/tasks/`.
5. Test Agents: failing tests first (Swift-style TDD principle adapted to Rust workspace).
6. Implementation Agents: core, knowledge, backends.
7. Review Agents: correctness, API misuse resistance, performance/concurrency safety.
8. Fix Agent: close all review gaps.
9. Final System Review Agent: holistic release readiness.

## 8. Timeline

1. Feb 10-16: context, baselines, plan freeze.
2. Feb 17-23: corpus lock, failing test harness complete.
3. Feb 24-Mar 9: parser/IR and Soroban semantics implementation.
4. Mar 10-18: WAT + Rust backends.
5. Mar 19-24: accuracy tuning and threshold pass.
6. Mar 25-28: reviews and gap closure.
7. Mar 29-31: docs, packaging, release candidate.

## 9. Deliverables

1. Open-source CLI + libraries.
2. Corpus and test fixtures.
3. Deterministic evaluation harness and reports.
4. Developer documentation and architecture notes.
5. Production-ready release artifacts.

## 10. Release Gates

1. Mandatory tests pass in CI.
2. Accuracy thresholds met.
3. CLI/library docs complete.
4. Security review for untrusted WASM parsing completed.
5. License, contribution guide, changelog present.

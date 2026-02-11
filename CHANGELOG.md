# Changelog

## 0.1.1 - 2026-02-10

1. Fixed WAT export rendering to emit valid targets (`func $symbol` / index-backed forms).
2. Added deterministic symbol/identifier disambiguation for colliding sanitized names in WAT and Rust backends.
3. Updated CLI `explain` to resolve opcode summaries from export-index mapping rather than symbol-name matching.
4. Updated `score` to use decompile-based Rust reconstruction for all corpus variants.
5. Tightened builtin coverage accounting to count only known Soroban env builtins as covered.
6. Regenerated locked corpus WASM artifacts and aligned source fixtures with deterministic reconstruction output.
7. Added corpus uniqueness tests and locked-corpus default-threshold CLI test coverage.
8. Added CI workflow to run workspace tests and locked-corpus threshold verification.

## 0.1.0 - 2026-02-10

1. Established immutable implementation plan and task/review artifacts.
2. Bootstrapped Rust workspace with six crates:
   - `sorcat-core`
   - `sorcat-soroban-knowledge`
   - `sorcat-wat-backend`
   - `sorcat-rust-backend`
   - `sorcat-eval`
   - `sorcat-cli`
3. Implemented baseline `sorcat-core` decoding/CFG/SSA/import-resolution APIs.
4. Added Soroban knowledge classification layer with deterministic resolution tests.
5. Implemented deterministic WAT and Rust summary backends.
6. Implemented CLI commands:
   - `decompile`
   - `score`
   - `explain`
   - `diff`
7. Added eval harness:
   - manifest/layout validation
   - AST normalization
   - AST-structure tree edit distance scoring
   - threshold evaluation
   - deterministic report rendering
8. Expanded locked corpus fixtures to plan-level counts and updated corpus tests.
9. Added release-process docs:
   - `LICENSE`
   - `CONTRIBUTING.md`
   - security review artifact

# Contributing to sorcat

## Scope

This repository contains an early implementation of the Soroban reverse engineering toolchain.
Contributions should prioritize:

1. Correctness
2. Type safety
3. API clarity
4. Performance (only when justified)

## Workflow

1. Open an issue describing the bug/feature and expected behavior.
2. Keep changes focused and deterministic.
3. Add/adjust tests with every behavior change.
4. Run full test suite before submitting.

```bash
cargo test --workspace --no-fail-fast
```

## Coding Guidelines

1. Avoid panics on untrusted WASM input paths.
2. Preserve deterministic output ordering in public render/report APIs.
3. Keep public API surface minimal and hard to misuse.
4. Prefer explicit error variants over stringly-typed failures.

## Pull Requests

Each PR should include:

1. Problem statement
2. Summary of behavior changes
3. Test evidence
4. Known follow-ups or non-goals

## Security

If you discover a security issue in parsing/decompilation behavior, open a private report first.
See `docs/security/untrusted-wasm-review-v1.md`.

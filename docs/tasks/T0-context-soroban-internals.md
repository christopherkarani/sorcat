Prompt:
Research and document Soroban-specific WASM structures and host-call conventions required for high-fidelity reverse engineering.

Goal:
Produce an implementation-ready Soroban knowledge mapping document for `sorcat-soroban-knowledge`.

Task Breakdown:
1. Enumerate Soroban host env functions and categorize by domain.
2. Identify WASM import/module naming conventions used by Soroban contracts.
3. Document XDR and SDK type signatures that must be reconstructed.
4. Define heuristic fallbacks when symbols/sections are stripped.
5. Capture versioning concerns and compatibility strategy.
6. Propose canonical internal representation for Soroban semantic hints.
7. Do not edit `docs/plans/sorcat-implementation-plan-v1.md`.

Expected Output:
1. `docs/context/soroban-internals-summary.md`
2. `docs/context/soroban-knowledge-schema.md`
3. Open questions list with blocking/non-blocking labels.

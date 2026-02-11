# Soroban Internals Summary (v1 Context)

## Scope
Implementation context for `sorcat-soroban-knowledge` covering:
- Soroban host internals
- host env/builtins mapping strategy
- XDR/SDK reconstruction targets
- v1 versioning assumptions

## Authoritative Sources
- `soroban-env-common` `env.json` (25.0.1) for host function catalog, module exports, function exports, protocol gating.
- `soroban-env-common` `trait Env` (25.0.1) for canonical method surface (171 required methods).
- `soroban-env-guest` `guest.rs` (25.0.1) for wasm import/link behavior (`wasm_import_module` + `link_name`).
- `soroban-env-common` `meta.rs` (25.0.1, 22.1.3) for interface/protocol compatibility behavior.
- `soroban-sdk` `contractmeta.rs` for contract custom section names and encodings.

## Host Env Structure
Soroban host functions are defined centrally in `env.json` and compiled into guest stubs.

WASM import wiring in guest stubs:
- import module is set by module export symbol (`#[link(wasm_import_module = $mod_str)]`)
- import field is set by function export symbol (`#[link_name = $fn_str]`)
- canonical function identity is **not** the wasm field name alone; it is resolved through `env.json`.

Observed module/export mapping in `env.json`:
- `context` -> `"x"`
- `int` -> `"i"`
- `map` -> `"m"`
- `vec` -> `"v"`
- `ledger` -> `"l"`
- `call` -> `"d"`
- `buf` -> `"b"`
- `crypto` -> `"c"`
- `address` -> `"a"`
- `test` -> `"t"`
- `prng` -> `"p"`

## Function Domains (Enumeration Strategy)
`Env` exposes 171 methods; implement enumeration from `env.json` at build/load time (not hardcoded), then map to these semantic domains:

1. Context/runtime:
`log_from_linear_memory`, `obj_cmp`, `contract_event`, ledger getters, `fail_with_error`, network/current-contract getters.

2. Integer/object conversions and arithmetic:
`obj_from_*`, `obj_to_*`, `u256_*`, `i256_*`, `*_val_from_be_bytes`, `*_val_to_be_bytes`, timepoint/duration object conversions.

3. Collections:
`map_*` and `vec_*` families, including linear-memory pack/unpack helpers.

4. Contract storage/lifecycle/deployment:
`put/get/del/has_contract_data`, `upload_wasm`, `update_current_contract_wasm`,
`create_contract`, `create_contract_with_constructor`, `create_asset_contract`,
`get_contract_id`, `get_asset_contract_id`, TTL extension functions.

5. Cross-contract invocation:
`call`, `try_call`.

6. Serialization/buffer bridging:
`serialize_to_bytes`, `deserialize_from_bytes`, bytes/string/symbol copy/new helpers.

7. Crypto:
SHA-256/Keccak, Ed25519/secp256k1/secp256r1 ops, BLS12-381 ops, BN254 ops, Poseidon/Poseidon2.

8. Address/auth:
`require_auth`, `require_auth_for_args`, strkey conversion, muxed-address helpers,
`authorize_as_curr_contract`, `get_address_executable`.

9. PRNG:
`prng_reseed`, `prng_bytes_new`, `prng_u64_in_inclusive_range`, `prng_vec_shuffle`.

10. Test/protocol gate sentinels:
`dummy0`, `protocol_gated_dummy`.

## Builtins Mapping Strategy (Implementation-Ready)
1. Build a generated knowledge table from upstream `env.json`:
   - key: `(wasm_import_module, wasm_import_field, protocol)`
   - value: canonical host function id + signature + semantics tags.
2. Resolve every wasm import using exact match on module+field.
3. Apply protocol gating via `min_supported_protocol` / `max_supported_protocol`.
4. Attach stable semantic family tags (`context`, `ledger_storage`, `crypto_hash`, etc.).
5. If section metadata is missing, infer protocol floor from used function minima.

Fallback policy for stripped/partial binaries:
- First fallback: arity/type-shape match within same module.
- Second fallback: callsite semantics (argument object types, follow-up operations).
- Never silently remap across modules when ambiguous; emit uncertain mapping with confidence.

## XDR/SDK Reconstruction Targets
### Required wasm custom sections
- `contractspecv0` -> decode as `Vec<ScSpecEntry>`.
- `contractmetav0` -> decode as `Vec<ScMetaEntry>`.
- `contractenvmetav0` -> decode as `ScEnvMetaEntry` (interface version).

### Required XDR surface to reconstruct
- Core value system: `ScVal`, `ScError`, `ScAddress`, `ScSymbol`, `ScMap`, `ScVec`.
- Contract description: `ScSpecEntry`, `ScSpecTypeDef`, `ScMetaEntry`.
- Invocation/deployment context: `HostFunction` variants (`InvokeContract`, `CreateContract`, `UploadContractWasm`, `CreateContractV2`) and related args types.

### SDK-level reconstruction goals
- Reconstruct contract function names/signatures from `ScSpecEntry`.
- Reconstruct user-defined contract types from `ScSpecTypeDef` graph.
- Reconstruct auth/address intent from address/auth host calls and argument shapes.

## Versioning Assumptions for v1.0
1. Baseline upstream references:
   - `soroban-env-common` 25.0.1: `ledger_protocol_version = 25`, `next = 26`.
   - `soroban-env-common` 22.1.3: `ledger_protocol_version = 22`.
2. v1 support window assumption:
   - stable protocol-era contracts from 22 through 25.
   - prerelease (`next`) interface variants are out of scope for strict guarantees.
3. Compatibility behavior:
   - host accepts same-or-older protocol; prerelease versions require exact match.
4. Knowledge bundle versioning:
   - freeze a generated `env.json`-derived bundle per supported protocol family.
   - include schema version + source digest for deterministic CI.

## Open Questions
- `BLOCKING`: Should v1 hard-fail on missing `contractenvmetav0`, or allow best-effort inference with downgraded confidence?
- `BLOCKING`: Which exact SDK versions are included in the locked corpus (minimum two are required by plan); propose explicit anchors `22.x` and `25.x`.
- `NON-BLOCKING`: Should test-only host fns (`dummy0`, `protocol_gated_dummy`) be retained in runtime mapping tables or excluded from user-facing outputs?
- `NON-BLOCKING`: Do we surface protocol-gated deprecations in CLI output (`explain`/`diff`) by default or behind verbose mode?

## References
- https://docs.rs/crate/soroban-env-common/latest/source/env.json
- https://docs.rs/soroban-env-common/latest/soroban_env_common/trait.Env.html
- https://docs.rs/crate/soroban-env-guest/latest/source/src/guest.rs
- https://docs.rs/crate/soroban-env-common/latest/source/src/meta.rs
- https://docs.rs/crate/soroban-env-common/22.1.3/source/src/meta.rs
- https://docs.rs/soroban-sdk/latest/src/soroban_sdk/contractmeta.rs.html

# Soroban Knowledge Schema (v1)

## Purpose
Define the canonical, deterministic schema consumed by `sorcat-soroban-knowledge` to map raw wasm imports and metadata into Soroban semantic hints.

## Design Goals
- Correct mapping under stripped symbols.
- Protocol-aware resolution.
- Deterministic serialization for CI reproducibility.
- Explicit confidence and evidence for non-exact matches.

## Top-Level Document
```yaml
schema_version: "1.0.0"
generated_at_utc: "ISO-8601"
sources:
  soroban_env_common_version: "25.0.1"
  env_json_sha256: "..."
  sdk_contractmeta_version: "25.0.1"
supported_protocols: [22, 23, 24, 25]
modules: [HostModule]
functions: [HostFunction]
custom_sections: [CustomSectionRule]
xdr_targets: [XdrTarget]
fallback_rules: [FallbackRule]
```

## Host Module Model
```yaml
HostModule:
  id: "context"             # stable semantic id
  wasm_import_module: "x"   # import module in contract wasm
  env_export_symbol: "x"    # source export symbol (from env.json)
  domain: "runtime_context" # coarse semantic category
```

## Host Function Model
```yaml
HostFunction:
  id: "context.log_from_linear_memory"   # stable canonical id
  canonical_name: "log_from_linear_memory"
  module_id: "context"
  wasm_import_module: "x"
  wasm_import_field: "0"                 # compact symbol from env.json
  min_protocol: 20
  max_protocol: null                      # null => open upper bound
  signature:
    params: ["Val", "Val", "u32"]
    result: "Void"
  tags: ["logging", "memory-read", "host-side-effect"]
  effects:
    traps: false
    reads_ledger: false
    writes_ledger: false
    nondeterministic: false
  reconstruction:
    wat_annotation: "soroban.context.log"
    rust_hint: "env.log(...)"
    ir_opcode: "HostCall"
```

## Custom Section Rules
```yaml
CustomSectionRule:
  section_name: "contractspecv0"
  decode_as: "Vec<ScSpecEntry>"
  required_for: ["function-signature-reconstruction", "type-reconstruction"]

CustomSectionRule:
  section_name: "contractenvmetav0"
  decode_as: "ScEnvMetaEntry"
  required_for: ["protocol/interface-version-resolution"]

CustomSectionRule:
  section_name: "contractmetav0"
  decode_as: "Vec<ScMetaEntry>"
  required_for: ["contract-metadata-reconstruction"]
```

## XDR Target Model
```yaml
XdrTarget:
  name: "ScVal"
  role: "core-value"
  priority: "P0"

XdrTarget:
  name: "ScSpecEntry"
  role: "contract-spec"
  priority: "P0"

XdrTarget:
  name: "HostFunction"
  role: "invoke-create-upload"
  priority: "P0"
```

## Fallback Rule Model
```yaml
FallbackRule:
  id: "module+arity+shape"
  when: "exact module/field miss or stripped import field"
  constraints:
    same_module_required: true
    protocol_compatible_only: true
  match_features:
    - param_count
    - val-vs-primitive shape
    - callsite usage pattern
  confidence:
    exact: 1.0
    inferred: 0.5-0.89
    ambiguous: <0.5
  action_on_ambiguous: "emit_unresolved_host_call"
```

## Runtime Resolution Contract
1. Parse wasm imports.
2. Exact lookup by `(module, field, protocol)` in `functions`.
3. If exact miss, run ordered `fallback_rules`.
4. Emit `SemanticHint` records with confidence and evidence.
5. Never coerce cross-module matches when multiple candidates remain.

## Semantic Hint Output (to `sorcat-core`)
```yaml
SemanticHint:
  ir_node_id: "..."
  kind: "HostCall"
  canonical_id: "ledger.get_contract_data"
  confidence: 1.0
  evidence:
    - "import module=x field=47"
    - "protocol=25 in supported range"
  protocol: 25
```

## Determinism Requirements
- Sort modules/functions by canonical id before serialization.
- Store source digests (`env_json_sha256`) and protocol window.
- Reject mixed-source bundles at load time unless explicitly allowed.

## Validation Checklist
- Every imported host call resolves to exactly one canonical id or unresolved-with-confidence.
- Protocol gating is enforced for every match.
- Section decoders validate XDR payloads and report structured decode errors.
- Knowledge pack checksum is stable across repeated generation.

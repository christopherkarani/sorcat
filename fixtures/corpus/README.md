# Locked Corpus Fixtures (v1)

This directory contains the committed locked corpus fixtures used by `sorcat-eval`.

Layout contract:
- `manifest.v1.json`: corpus schema and contract inventory.
- `contracts/<category>/<contract_id>/src/lib.rs`: source fixture for the contract.
- `contracts/<category>/<contract_id>/wasm/*.wasm`: compiled wasm variants.
- `contracts/<category>/<contract_id>/metadata.json`: fixture metadata.
  - `real_world` metadata includes `source_provenance` fields (`upstream_repo_url`, `upstream_commit`, `upstream_license`, `source_origin`, `build_recipe`).

Corpus guarantees:
- 40 total contracts (20 `real_world`, 10 `synthetic`, 10 `adversarial`).
- Variant matrix coverage includes both `debug` and `release`.
- Variant matrix coverage includes both debug names included/excluded modes.
- SDK version coverage includes at least two versions (`22.1.0` and `25.0.1`).
- Every declared wasm path resolves to a committed wasm binary artifact.

Ordering guarantees:
- Manifest contract entries are committed in deterministic category/id order.
- Fixture files use stable names and paths to keep test assertions deterministic.

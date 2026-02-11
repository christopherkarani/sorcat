mod support;

use sorcat_core::{SorobanImportKind, SorobanImportResolution, resolve_soroban_imports};
use support::load_wasm_fixture;

#[test]
fn classifies_soroban_imports_into_builtin_unknown_and_non_env_groups() {
    let wasm = load_wasm_fixture("soroban_env_imports.wasm");

    let imports = resolve_soroban_imports(&wasm)
        .expect("Soroban import resolution should succeed for env import fixture");

    assert_kind(&imports, "env", "vec_new", SorobanImportKind::EnvBuiltin);
    assert_kind(&imports, "env", "map_new", SorobanImportKind::EnvBuiltin);
    assert_kind(
        &imports,
        "env",
        "prng_vec_shuffle",
        SorobanImportKind::EnvBuiltin,
    );
    assert_kind(
        &imports,
        "env",
        "mystery_host_fn",
        SorobanImportKind::EnvUnknown,
    );
    assert_kind(&imports, "not_env", "helper", SorobanImportKind::NonEnv);
}

#[test]
fn returns_import_resolution_in_deterministic_order() {
    let wasm = load_wasm_fixture("soroban_env_imports.wasm");

    let imports = resolve_soroban_imports(&wasm)
        .expect("Soroban import resolution should succeed for determinism checks");

    let rendered: Vec<String> = imports
        .iter()
        .map(|import| format!("{}::{}", import.module, import.name))
        .collect();
    let mut sorted = rendered.clone();
    sorted.sort();

    assert_eq!(
        rendered, sorted,
        "import resolution output must be deterministic (lexicographically sorted)",
    );
}

fn assert_kind(
    imports: &[SorobanImportResolution],
    module: &str,
    name: &str,
    expected_kind: SorobanImportKind,
) {
    let resolved = imports
        .iter()
        .find(|import| import.module == module && import.name == name)
        .unwrap_or_else(|| panic!("missing import classification for {module}::{name}"));

    assert_eq!(
        resolved.kind, expected_kind,
        "unexpected classification for {module}::{name}",
    );
}

use std::fs;
use std::path::PathBuf;

pub fn load_wasm_fixture(name: &str) -> Vec<u8> {
    let path = fixture_path(name);
    fs::read(&path)
        .unwrap_or_else(|err| panic!("failed to read WASM fixture {}: {err}", path.display()))
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/wasm")
        .join(name)
}

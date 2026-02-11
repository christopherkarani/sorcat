use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root must be two levels up from crate manifest")
        .to_path_buf()
}

pub fn corpus_root() -> PathBuf {
    workspace_root().join("fixtures").join("corpus")
}

pub fn corpus_manifest_path() -> PathBuf {
    corpus_root().join("manifest.v1.json")
}

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);

        let path = std::env::temp_dir().join(format!(
            "sorcat-eval-{prefix}-{}-{nanos}-{unique}",
            std::process::id()
        ));

        fs::create_dir_all(&path).expect("temporary test directory should be created");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn write_text(path: impl AsRef<Path>, text: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should be created");
    }
    fs::write(path, text).expect("text fixture should be written");
}

pub fn write_wasm_header(path: impl AsRef<Path>) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should be created");
    }
    let wasm_header = [0x00u8, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    fs::write(path, wasm_header).expect("wasm fixture should be written");
}

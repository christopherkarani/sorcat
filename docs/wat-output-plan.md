# Valid WAT Output Plan (Offline-Friendly)

## Goal
Produce *real* WAT from WASM bytes (full disassembly), and optionally add Soroban-aware annotations, while being resilient to `crates.io` index/DNS outages.

## Constraints Observed Here
- `index.crates.io` and `github.com` DNS resolution is unavailable in this environment, so we cannot fetch new crates today.
- This means any approach that introduces a new registry dependency (like `wasmprinter`) must also include a vendoring strategy.

## Recommended Approach
1. **Adopt vendoring as the default build mode** (CI and local):
   - On a networked machine, run `cargo vendor` for the workspace.
   - Commit the `vendor/` directory.
   - Add `.cargo/config.toml` to replace `crates-io` with the vendored source.
   - Build/test with `--offline` + `--locked` (or `--frozen`) so index outages cannot break builds.
2. **Use the upstream `wasmprinter` crate** to print real WAT:
   - It is the canonical upstream printer used by `wasm-tools`.
   - Keep the existing deterministic "summary WAT" APIs unchanged (they remain useful for stable diffs/scoring).
   - Add a new API `render_wat_from_wasm(...)` that returns full WAT.
3. **Soroban annotations strategy**:
   - Keep annotations *pure comments* so the output remains valid WAT.
   - Start with a deterministic prelude block:
     - Resolve imports with `sorcat-soroban-knowledge` (canonical ids when known).
     - Emit as `;;` comment lines prepended to the WAT.
   - Optionally add an inline mode later:
     - Post-process the printed WAT and append `;; soroban: ...` to matching `(import ...)` lines.

## Minimal Patch (When Vendoring Is In Place)

### 1) Workspace `Cargo.toml`
Add the dependency version:

```diff
diff --git a/Cargo.toml b/Cargo.toml
--- a/Cargo.toml
+++ b/Cargo.toml
@@
 [workspace.dependencies]
@@
 wasmparser = "0.240"
+wasmprinter = "0.240"
 wat = "1.240"
```

### 2) `crates/sorcat-wat-backend/Cargo.toml`
Add `wasmprinter` and knowledge dependency:

```diff
diff --git a/crates/sorcat-wat-backend/Cargo.toml b/crates/sorcat-wat-backend/Cargo.toml
--- a/crates/sorcat-wat-backend/Cargo.toml
+++ b/crates/sorcat-wat-backend/Cargo.toml
@@
 [dependencies]
 sorcat-core = { path = "../sorcat-core" }
+wasmprinter = { workspace = true }
+sorcat-soroban-knowledge = { path = "../sorcat-soroban-knowledge" }
```

### 3) `crates/sorcat-wat-backend/src/lib.rs`
Add a new printer API + annotations prelude (keeping existing summary APIs intact):

```diff
diff --git a/crates/sorcat-wat-backend/src/lib.rs b/crates/sorcat-wat-backend/src/lib.rs
--- a/crates/sorcat-wat-backend/src/lib.rs
+++ b/crates/sorcat-wat-backend/src/lib.rs
@@
 use sorcat_core::{
     CoreError, DecodedModuleSummary, Export, ExportKind, FunctionBodySummary, ImportFunction,
     decode_module_summary,
 };
+use sorcat_soroban_knowledge::{
+    SorobanSymbolKind,
+    resolve_imports as resolve_soroban_knowledge_imports,
+};
@@
 pub enum WatBackendError {
     Core(CoreError),
+    WasmPrinter { message: String },
     InvalidInput {
         field: &'static str,
         message: String,
     },
 }
@@
 impl Display for WatBackendError {
@@
         match self {
             Self::Core(source) => write!(f, "core decode failed: {}", source.message),
+            Self::WasmPrinter { message } => write!(f, "wasmprinter failed: {message}"),
             Self::InvalidInput { field, message } => {
                 write!(f, "invalid input for `{field}`: {message}")
             }
         }
     }
 }
@@
+pub fn render_wat_from_wasm(wasm: &[u8]) -> Result<String, WatBackendError> {
+    wasmprinter::print_bytes(wasm).map_err(|error| WatBackendError::WasmPrinter {
+        message: error.to_string(),
+    })
+}
+
+pub fn render_wat_from_wasm_with_soroban_annotations(
+    wasm: &[u8],
+) -> Result<String, WatBackendError> {
+    let wat = render_wat_from_wasm(wasm)?;
+    let Some(annotations) = render_soroban_annotations(wasm) else {
+        return Ok(wat);
+    };
+    Ok(format!("{annotations}\\n{wat}"))
+}
+
+fn render_soroban_annotations(wasm: &[u8]) -> Option<String> {
+    let summary = decode_module_summary(wasm).ok()?;
+    let resolved = resolve_soroban_knowledge_imports(
+        summary
+            .import_functions
+            .iter()
+            .map(|import| (import.module.clone(), import.name.clone())),
+    );
+    if resolved.is_empty() {
+        return None;
+    }
+
+    let mut lines = Vec::with_capacity(resolved.len() + 2);
+    lines.push(\";; sorcat soroban annotations v0\".to_string());
+    lines.push(\";; resolved imports:\".to_string());
+    for entry in resolved {
+        let kind = match entry.kind {
+            SorobanSymbolKind::EnvBuiltin => \"EnvBuiltin\",
+            SorobanSymbolKind::EnvUnknown => \"EnvUnknown\",
+            SorobanSymbolKind::NonEnv => \"NonEnv\",
+        };
+        let target = entry.canonical_id.as_deref().unwrap_or(\"<unknown>\");
+        lines.push(format!(
+            \";;   {}::{} -> {} ({kind})\",
+            entry.module, entry.name, target
+        ));
+    }
+    Some(lines.join(\"\\n\"))
+}
```

## Vendoring Notes
If you commit a full `vendor/` directory, add a `.cargo/config.toml` like:

```toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

Then CI can run:
- `cargo test --locked --offline`


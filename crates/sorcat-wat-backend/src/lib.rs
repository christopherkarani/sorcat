use std::error::Error;
use std::fmt::{Display, Formatter};
use std::collections::BTreeMap;

use sorcat_core::{
    CoreError, DecodedModuleSummary, Export, ExportKind, FunctionBodySummary, ImportFunction,
    SorobanCustomSections, decode_module_summary, decode_soroban_custom_sections,
};
use sorcat_soroban_knowledge::{
    SorobanSymbolKind, resolve_imports as resolve_soroban_knowledge_imports,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatBackendError {
    Core(CoreError),
    WasmPrinter {
        message: String,
    },
    InvalidInput {
        field: &'static str,
        message: String,
    },
}

impl Display for WatBackendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(source) => write!(f, "core decode failed: {}", source.message),
            Self::WasmPrinter { message } => write!(f, "wasmprinter failed: {message}"),
            Self::InvalidInput { field, message } => {
                write!(f, "invalid input for `{field}`: {message}")
            }
        }
    }
}

impl Error for WatBackendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl From<CoreError> for WatBackendError {
    fn from(value: CoreError) -> Self {
        Self::Core(value)
    }
}

pub fn render_module_summary_from_wasm(wasm: &[u8]) -> Result<String, WatBackendError> {
    let summary = decode_module_summary(wasm)?;
    render_module_summary(&summary)
}

/// Render a full WebAssembly Text (WAT) representation using the upstream `wasmprinter` crate.
pub fn render_wat_from_wasm(wasm: &[u8]) -> Result<String, WatBackendError> {
    wasmprinter::print_bytes(wasm).map_err(|error| WatBackendError::WasmPrinter {
        message: error.to_string(),
    })
}

/// Render full WAT with a deterministic comment prelude containing Soroban import annotations.
pub fn render_wat_from_wasm_with_soroban_annotations(
    wasm: &[u8],
) -> Result<String, WatBackendError> {
    let wat = render_wat_from_wasm(wasm)?;
    let summary = decode_module_summary(wasm)?;
    let soroban_sections = decode_soroban_custom_sections(wasm)?;
    let Some(annotations) = render_soroban_annotations(&summary, &soroban_sections) else {
        return Ok(wat);
    };

    Ok(format!("{annotations}\n{wat}"))
}

pub fn render_module_summary(summary: &DecodedModuleSummary) -> Result<String, WatBackendError> {
    validate_summary(summary)?;

    let mut imports = summary.import_functions.clone();
    imports.sort_by(|left, right| {
        left.module
            .cmp(&right.module)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.params.cmp(&right.params))
            .then_with(|| left.results.cmp(&right.results))
    });

    let mut exports = summary.exports.clone();
    exports.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| export_kind_tag(left.kind).cmp(export_kind_tag(right.kind)))
    });

    let mut bodies = summary.function_bodies.clone();
    bodies.sort_by(|left, right| left.symbol.cmp(&right.symbol));

    let mut used_symbols = BTreeMap::<String, usize>::new();
    let mut rendered_imports = Vec::with_capacity(imports.len());
    for import in imports {
        let base_symbol = sanitize_symbol(&format!("{}_{}", import.module, import.name));
        let symbol = make_unique_symbol(base_symbol, &mut used_symbols);
        rendered_imports.push(render_import_line(import, symbol));
    }

    let mut function_targets = BTreeMap::<u32, String>::new();
    let mut rendered_bodies = Vec::with_capacity(bodies.len());
    for body in bodies {
        let base_symbol = sanitize_symbol(&body.symbol);
        let symbol = make_unique_symbol(base_symbol, &mut used_symbols);
        function_targets
            .entry(body.function_index)
            .or_insert_with(|| symbol.clone());
        rendered_bodies.push(render_function_line(body, symbol));
    }

    let mut lines = Vec::new();
    lines.push(";; sorcat deterministic WAT summary v0".to_string());
    lines.push("(module".to_string());
    lines.push(format!(
        "  ;; defined function bodies: {}",
        summary.defined_function_bodies
    ));
    lines.push(format!("  ;; imports: {}", summary.import_functions.len()));
    lines.extend(rendered_imports);
    lines.push(format!("  ;; exports: {}", exports.len()));
    lines.extend(
        exports
            .into_iter()
            .map(|export| render_export_line(export, &function_targets)),
    );
    lines.push(format!("  ;; function summaries: {}", summary.function_bodies.len()));
    lines.extend(rendered_bodies);
    lines.push(")".to_string());

    Ok(lines.join("\n"))
}

fn render_soroban_annotations(
    summary: &DecodedModuleSummary,
    soroban_sections: &SorobanCustomSections,
) -> Option<String> {
    let resolved = resolve_soroban_knowledge_imports(
        summary
            .import_functions
            .iter()
            .map(|import| (import.module.clone(), import.name.clone())),
    );

    let has_custom_semantics = soroban_sections.contract_spec.is_some()
        || soroban_sections.contract_meta.is_some()
        || soroban_sections.contract_env_meta.is_some();

    if resolved.is_empty() && !has_custom_semantics {
        return None;
    }

    let mut lines = Vec::new();
    lines.push(";; sorcat soroban annotations v1".to_string());
    lines.push(format!(";; imports={}", resolved.len()));

    for entry in resolved {
        let kind = match entry.kind {
            SorobanSymbolKind::EnvBuiltin => "env_builtin",
            SorobanSymbolKind::EnvUnknown => "env_unknown",
            SorobanSymbolKind::NonEnv => "non_env",
        };
        let canonical = entry.canonical_id.as_deref().unwrap_or("<none>");
        let signature = entry
            .signature
            .as_ref()
            .map(|sig| format!("({}) -> {}", sig.params.join(", "), sig.result))
            .unwrap_or_else(|| "<unknown>".to_string());
        let protocol = match (entry.min_protocol, entry.max_protocol) {
            (Some(min), Some(max)) => format!("{min}..={max}"),
            _ => "<none>".to_string(),
        };
        lines.push(format!(
            ";; - {}::{} [{}] canonical_id={} protocol={} confidence={} reason={} signature={}",
            entry.module,
            entry.name,
            kind,
            canonical,
            protocol,
            entry.confidence,
            entry.reason,
            signature
        ));
        if !entry.semantic_tags.is_empty() {
            lines.push(format!(
                ";;   tags=[{}]",
                entry.semantic_tags.join(", ")
            ));
        }
    }

    if let Some(spec) = &soroban_sections.contract_spec {
        lines.push(format!(
            ";; custom.contractspecv0 functions={} types={} errors={}",
            spec.functions.len(),
            spec.types.len(),
            spec.errors.len()
        ));
        for ty in &spec.types {
            lines.push(format!(
                ";;   type_hint {} kind={}",
                ty.name,
                spec_type_kind_name(ty.kind)
            ));
        }
        for error in &spec.errors {
            lines.push(format!(";;   error_hint {} code={}", error.name, error.code));
        }
    }

    if let Some(meta) = &soroban_sections.contract_meta {
        let contract_name = meta.contract_name.as_deref().unwrap_or("<unknown>");
        let version = meta.version.as_deref().unwrap_or("<unknown>");
        lines.push(format!(
            ";; custom.contractmetav0 contract_name={} version={} entries={}",
            contract_name,
            version,
            meta.entries.len()
        ));
    }

    if let Some(env_meta) = &soroban_sections.contract_env_meta {
        lines.push(format!(
            ";; custom.contractenvmetav0 protocol={} interface_version={} sdk_version={}",
            env_meta.protocol,
            env_meta
                .interface_version
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            env_meta
                .sdk_version
                .as_deref()
                .unwrap_or("<none>")
        ));
    }

    Some(lines.join("\n"))
}

fn validate_summary(summary: &DecodedModuleSummary) -> Result<(), WatBackendError> {
    if summary.defined_function_bodies != summary.function_bodies.len() {
        return Err(WatBackendError::InvalidInput {
            field: "decoded_module_summary.defined_function_bodies",
            message: "count mismatch with function_bodies length".to_string(),
        });
    }

    for import in &summary.import_functions {
        if import.module.trim().is_empty() {
            return Err(WatBackendError::InvalidInput {
                field: "decoded_module_summary.import_functions.module",
                message: "module must not be empty".to_string(),
            });
        }
        if import.name.trim().is_empty() {
            return Err(WatBackendError::InvalidInput {
                field: "decoded_module_summary.import_functions.name",
                message: "name must not be empty".to_string(),
            });
        }
    }

    for export in &summary.exports {
        if export.name.trim().is_empty() {
            return Err(WatBackendError::InvalidInput {
                field: "decoded_module_summary.exports.name",
                message: "name must not be empty".to_string(),
            });
        }
    }

    for body in &summary.function_bodies {
        if body.symbol.trim().is_empty() {
            return Err(WatBackendError::InvalidInput {
                field: "decoded_module_summary.function_bodies.symbol",
                message: "symbol must not be empty".to_string(),
            });
        }
    }

    Ok(())
}

fn render_import_line(import: ImportFunction, symbol: String) -> String {
    let params = render_type_list(&import.params, "param");
    let results = render_type_list(&import.results, "result");
    format!(
        "  (import \"{}\" \"{}\" (func ${symbol}{params}{results}))",
        escape_quotes(&import.module),
        escape_quotes(&import.name),
    )
}

fn render_export_line(
    export: Export,
    function_targets: &BTreeMap<u32, String>,
) -> String {
    let target = match export.kind {
        ExportKind::Function => function_targets
            .get(&export.index)
            .map(|symbol| format!("func ${symbol}"))
            .unwrap_or_else(|| format!("func {}", export.index)),
        ExportKind::Memory => format!("memory {}", export.index),
        ExportKind::Table => format!("table {}", export.index),
        ExportKind::Global => format!("global {}", export.index),
    };

    format!(
        "  (export \"{}\" ({}))",
        escape_quotes(&export.name),
        target,
    )
}

fn render_function_line(body: FunctionBodySummary, symbol: String) -> String {
    let opcodes = if body.opcodes.is_empty() {
        "<none>".to_string()
    } else {
        body.opcodes.join(", ")
    };
    format!("  (func ${symbol} ;; opcodes: {opcodes})")
}

fn render_type_list(types: &[String], group_tag: &str) -> String {
    if types.is_empty() {
        return String::new();
    }
    format!(" ({group_tag} {})", types.join(" "))
}

fn export_kind_tag(kind: ExportKind) -> &'static str {
    match kind {
        ExportKind::Function => "func",
        ExportKind::Memory => "memory",
        ExportKind::Table => "table",
        ExportKind::Global => "global",
    }
}

fn spec_type_kind_name(kind: sorcat_core::SorobanSpecTypeKind) -> &'static str {
    match kind {
        sorcat_core::SorobanSpecTypeKind::Struct => "struct",
        sorcat_core::SorobanSpecTypeKind::Enum => "enum",
        sorcat_core::SorobanSpecTypeKind::Alias => "alias",
    }
}

fn make_unique_symbol(base: String, counts: &mut BTreeMap<String, usize>) -> String {
    match counts.get_mut(&base) {
        Some(next_suffix) => {
            let symbol = format!("{base}_{next_suffix}");
            *next_suffix += 1;
            symbol
        }
        None => {
            counts.insert(base.clone(), 1);
            base
        }
    }
}

fn sanitize_symbol(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "symbol".to_string()
    } else if output
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        format!("s_{output}")
    } else {
        output
    }
}

fn escape_quotes(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sorcat_core::{
        DecodedModuleSummary, Export, ExportKind, FunctionBodySummary, ImportFunction,
        decode_module_summary,
    };
    use sorcat_soroban_knowledge::{
        SorobanSymbolKind, resolve_imports as resolve_soroban_knowledge_imports,
    };

    use super::{
        WatBackendError,
        render_module_summary,
        render_module_summary_from_wasm,
        render_wat_from_wasm,
        render_wat_from_wasm_with_soroban_annotations,
    };

    #[test]
    fn render_from_wasm_is_deterministic_for_fixture() {
        let wasm = load_wasm_fixture("sections_imports_exports.wasm");

        let first = render_module_summary_from_wasm(&wasm)
            .expect("rendering should succeed for deterministic fixture");
        let second = render_module_summary_from_wasm(&wasm)
            .expect("rendering should succeed for deterministic fixture");

        assert_eq!(first, second, "rendering must be deterministic");
        assert!(
            first.contains("(import \"env\" \"log\""),
            "expected env::log import in WAT summary"
        );
        assert!(
            first.contains("(export \"adder\" (func $adder))"),
            "expected adder export in WAT summary"
        );
        assert!(
            first.contains("(func $adder ;; opcodes: local.get, local.get, i32.add, end)"),
            "expected deterministic opcode line for adder"
        );
    }

    #[test]
    fn render_summary_sorts_unsorted_inputs() {
        let summary = DecodedModuleSummary {
            import_functions: vec![
                ImportFunction {
                    module: "env".to_string(),
                    name: "z".to_string(),
                    params: vec!["i32".to_string()],
                    results: vec![],
                },
                ImportFunction {
                    module: "env".to_string(),
                    name: "a".to_string(),
                    params: vec![],
                    results: vec!["i64".to_string()],
                },
            ],
            exports: vec![
                Export {
                    name: "zeta".to_string(),
                    kind: ExportKind::Function,
                    index: 1,
                },
                Export {
                    name: "alpha".to_string(),
                    kind: ExportKind::Memory,
                    index: 0,
                },
            ],
            defined_function_bodies: 2,
            function_bodies: vec![
                FunctionBodySummary {
                    function_index: 1,
                    symbol: "zeta".to_string(),
                    opcodes: vec!["end".to_string()],
                    instructions: vec![],
                },
                FunctionBodySummary {
                    function_index: 2,
                    symbol: "alpha".to_string(),
                    opcodes: vec!["local.get".to_string(), "end".to_string()],
                    instructions: vec![],
                },
            ],
        };

        let rendered =
            render_module_summary(&summary).expect("summary rendering should support direct input");
        let lines: Vec<&str> = rendered.lines().collect();

        let first_import = lines
            .iter()
            .position(|line| line.contains("(import \"env\" \"a\""))
            .expect("missing env::a import");
        let second_import = lines
            .iter()
            .position(|line| line.contains("(import \"env\" \"z\""))
            .expect("missing env::z import");
        assert!(
            first_import < second_import,
            "imports should be sorted lexicographically"
        );
    }

    #[test]
    fn render_full_wat_from_wasm_is_parseable_and_contains_expected_import_export() {
        let wasm = load_wasm_fixture("sections_imports_exports.wasm");

        let rendered = render_wat_from_wasm(&wasm).expect("wasmprinter should render fixture");
        assert!(
            rendered.trim_start().starts_with("(module"),
            "expected wasmprinter to emit a WAT module"
        );
        assert!(
            rendered.contains("(import \"env\" \"log\""),
            "expected env::log import in printed WAT"
        );
        assert!(
            rendered.contains("(export \"adder\""),
            "expected adder export in printed WAT"
        );
    }

    #[test]
    fn render_full_wat_includes_soroban_annotations_prelude() {
        let wasm = load_wasm_fixture("soroban_env_imports.wasm");

        let rendered = render_wat_from_wasm_with_soroban_annotations(&wasm)
            .expect("wasmprinter + annotations should render fixture");
        assert!(
            rendered.contains(";; sorcat soroban annotations v1"),
            "expected soroban annotations prelude"
        );
        assert!(
            rendered.contains("confidence=") && rendered.contains("protocol="),
            "expected richer builtin annotation fields for confidence/protocol"
        );

        // Ensure at least one builtin canonical id is present in the rendered comment block.
        let summary = decode_module_summary(&wasm).expect("fixture should decode");
        let resolved = resolve_soroban_knowledge_imports(
            summary
                .import_functions
                .iter()
                .map(|import| (import.module.clone(), import.name.clone())),
        );
        let builtin = resolved
            .iter()
            .find(|entry| entry.kind == SorobanSymbolKind::EnvBuiltin && entry.canonical_id.is_some())
            .and_then(|entry| entry.canonical_id.as_ref())
            .expect("expected at least one EnvBuiltin import with a canonical id");
        assert!(
            rendered.contains(builtin),
            "expected canonical id {builtin} to appear in annotations"
        );
    }

    #[test]
    fn render_full_wat_includes_custom_section_summaries_and_type_error_hints() {
        let wasm = wasm_with_soroban_custom_sections();

        let first = render_wat_from_wasm_with_soroban_annotations(&wasm)
            .expect("annotated WAT should render for custom section fixture");
        let second = render_wat_from_wasm_with_soroban_annotations(&wasm)
            .expect("annotated WAT should be deterministic");

        assert_eq!(first, second, "annotation rendering must be deterministic");
        assert!(
            first.contains("custom.contractspecv0 functions=1 types=2 errors=1"),
            "expected decoded contractspec summary annotation"
        );
        assert!(
            first.contains("type_hint Allowance kind=struct"),
            "expected type hint annotation from decoded spec"
        );
        assert!(
            first.contains("error_hint InsufficientBalance code=1"),
            "expected error hint annotation from decoded spec"
        );
        assert!(
            first.contains("custom.contractmetav0 contract_name=token"),
            "expected contract metadata annotation"
        );
        assert!(
            first.contains("custom.contractenvmetav0 protocol=25"),
            "expected env metadata annotation"
        );
    }

    #[test]
    fn render_summary_rejects_invalid_inputs() {
        let summary = DecodedModuleSummary {
            import_functions: vec![],
            exports: vec![],
            defined_function_bodies: 1,
            function_bodies: vec![],
        };

        let error = render_module_summary(&summary)
            .expect_err("count mismatch should fail with structured input error");
        assert!(
            matches!(error, WatBackendError::InvalidInput { .. }),
            "expected InvalidInput for summary mismatch"
        );
    }

    #[test]
    fn render_summary_disambiguates_colliding_symbols() {
        let summary = DecodedModuleSummary {
            import_functions: vec![
                ImportFunction {
                    module: "env".to_string(),
                    name: "host-fn".to_string(),
                    params: vec![],
                    results: vec![],
                },
                ImportFunction {
                    module: "env".to_string(),
                    name: "host_fn".to_string(),
                    params: vec![],
                    results: vec![],
                },
            ],
            exports: vec![Export {
                name: "f".to_string(),
                kind: ExportKind::Function,
                index: 1,
            }],
            defined_function_bodies: 2,
            function_bodies: vec![
                FunctionBodySummary {
                    function_index: 1,
                    symbol: "fn-a".to_string(),
                    opcodes: vec!["end".to_string()],
                    instructions: vec![],
                },
                FunctionBodySummary {
                    function_index: 2,
                    symbol: "fn_a".to_string(),
                    opcodes: vec!["end".to_string()],
                    instructions: vec![],
                },
            ],
        };

        let rendered = render_module_summary(&summary).expect("rendering should succeed");
        assert!(
            rendered.contains("(import \"env\" \"host-fn\" (func $env_host_fn))"),
            "first colliding import should keep base symbol"
        );
        assert!(
            rendered.contains("(import \"env\" \"host_fn\" (func $env_host_fn_1))"),
            "second colliding import should receive deterministic suffix"
        );
        assert!(
            rendered.contains("(func $fn_a ;; opcodes: end)"),
            "first colliding function symbol should keep base form"
        );
        assert!(
            rendered.contains("(func $fn_a_1 ;; opcodes: end)"),
            "second colliding function symbol should receive deterministic suffix"
        );
    }

    fn load_wasm_fixture(name: &str) -> Vec<u8> {
        let path = fixture_path(name);
        std::fs::read(&path)
            .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()))
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/wasm")
            .join(name)
    }

    fn wasm_with_soroban_custom_sections() -> Vec<u8> {
        let spec = b"\nfn|transfer||i64|Transfer tokens\ntype|struct|Allowance|spender:Address,amount:i128\ntype|enum|Role|Admin,User\nerror|InsufficientBalance|1|Not enough funds\n";
        let meta = b"\ncontract_name=token\nversion=1.0.0\nentry|authors|stellar\n";
        let env_meta = b"\nprotocol=25\ninterface_version=1\nsdk_version=25.0.1\n";

        let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        wasm.extend_from_slice(&wasm_custom_section("contractspecv0", spec));
        wasm.extend_from_slice(&wasm_custom_section("contractmetav0", meta));
        wasm.extend_from_slice(&wasm_custom_section("contractenvmetav0", env_meta));
        wasm.extend_from_slice(&[
            0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7e, // type section: () -> i64
            0x03, 0x02, 0x01, 0x00, // function section
            0x07, 0x0c, 0x01, 0x08, b't', b'r', b'a', b'n', b's', b'f', b'e', b'r', 0x00, 0x00, // export
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x42, 0x01, 0x0b, // code section
        ]);
        wasm
    }

    fn wasm_custom_section(name: &str, payload_data: &[u8]) -> Vec<u8> {
        let mut payload = encode_name(name);
        payload.extend_from_slice(payload_data);

        let mut section = Vec::new();
        section.push(0x00); // custom section id
        push_leb_u32(payload.len() as u32, &mut section);
        section.extend_from_slice(&payload);
        section
    }

    fn encode_name(name: &str) -> Vec<u8> {
        let bytes = name.as_bytes();
        let mut out = Vec::new();
        push_leb_u32(bytes.len() as u32, &mut out);
        out.extend_from_slice(bytes);
        out
    }

    fn push_leb_u32(mut value: u32, output: &mut Vec<u8>) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            output.push(byte);
            if value == 0 {
                break;
            }
        }
    }
}

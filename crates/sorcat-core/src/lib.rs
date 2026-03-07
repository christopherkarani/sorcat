use std::collections::BTreeMap;

use sorcat_soroban_knowledge::{resolve_imports as resolve_knowledge_imports, SorobanSymbolKind};
use wasmparser::{Parser, Payload};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Function,
    Memory,
    Table,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFunction {
    pub module: String,
    pub name: String,
    pub params: Vec<String>,
    pub results: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
}

/// A decoded instruction stream for a function body.
///
/// This is intentionally a small, deterministic subset of the WebAssembly
/// instruction set that `sorcat-core` currently supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Block,
    Loop,
    If,
    Else,
    End,
    Br {
        depth: u32,
    },
    BrIf {
        depth: u32,
    },
    Call {
        function_index: u32,
    },
    CallIndirect {
        type_index: u32,
        table_index: u32,
    },
    LocalGet {
        local_index: u32,
    },
    LocalSet {
        local_index: u32,
    },
    LocalTee {
        local_index: u32,
    },
    GlobalGet {
        global_index: u32,
    },
    GlobalSet {
        global_index: u32,
    },
    I32Const {
        value: i32,
    },
    I64Const {
        value: i64,
    },
    I32Eqz,
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,
    I64Eqz,
    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    Select,
    BrTable {
        targets: Vec<u32>,
        default_target: u32,
    },
    Drop,
    Return,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionBodySummary {
    pub function_index: u32,
    pub symbol: String,
    pub params: Vec<String>,
    pub results: Vec<String>,
    pub opcodes: Vec<String>,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedModuleSummary {
    pub import_functions: Vec<ImportFunction>,
    pub exports: Vec<Export>,
    pub defined_function_bodies: usize,
    pub function_bodies: Vec<FunctionBodySummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Fallthrough,
    BranchTrue,
    BranchFalse,
    BackEdge,
    Unconditional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgSummary {
    pub entry: String,
    pub blocks: Vec<String>,
    pub edges: Vec<CfgEdge>,
    pub exits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaSummary {
    pub params: Vec<String>,
    pub instructions: Vec<String>,
    pub phi_nodes: usize,
    pub terminator: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SorobanImportKind {
    EnvBuiltin,
    EnvUnknown,
    NonEnv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanImportResolution {
    pub module: String,
    pub name: String,
    pub kind: SorobanImportKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    pub max_wasm_bytes: usize,
    pub max_instructions_per_function: usize,
    pub max_block_nesting_depth: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_wasm_bytes: 16 * 1024 * 1024,
            max_instructions_per_function: 250_000,
            max_block_nesting_depth: 4_096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanCustomSections {
    pub contract_spec: Option<SorobanContractSpec>,
    pub contract_meta: Option<SorobanContractMeta>,
    pub contract_env_meta: Option<SorobanContractEnvMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanContractSpec {
    pub functions: Vec<SorobanSpecFunction>,
    pub types: Vec<SorobanSpecType>,
    pub errors: Vec<SorobanSpecError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanSpecFunction {
    pub name: String,
    pub inputs: Vec<SorobanSpecField>,
    pub output: Option<String>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanSpecField {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SorobanSpecTypeKind {
    Struct,
    Enum,
    Alias,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanSpecType {
    pub name: String,
    pub kind: SorobanSpecTypeKind,
    pub fields: Vec<SorobanSpecField>,
    pub variants: Vec<String>,
    pub alias_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanSpecError {
    pub name: String,
    pub code: i32,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanContractMeta {
    pub contract_name: Option<String>,
    pub version: Option<String>,
    pub entries: Vec<SorobanMetaEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanMetaEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanContractEnvMeta {
    pub protocol: u32,
    pub interface_version: Option<u32>,
    pub sdk_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreErrorKind {
    MalformedBinary,
    UnsupportedConstruct,
    ResourceLimitExceeded,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreError {
    pub kind: CoreErrorKind,
    pub message: String,
}

pub type CoreResult<T> = Result<T, CoreError>;

pub fn decode_module_summary(wasm: &[u8]) -> CoreResult<DecodedModuleSummary> {
    decode_module_summary_with_limits(wasm, &ParseLimits::default())
}

pub fn decode_module_summary_with_limits(
    wasm: &[u8],
    limits: &ParseLimits,
) -> CoreResult<DecodedModuleSummary> {
    validate_wasm_with_limits(wasm, limits)?;
    let parsed = parse_module(wasm, limits)?;

    let mut import_functions: Vec<ImportFunction> =
        Vec::with_capacity(parsed.imported_functions.len());
    for import in &parsed.imported_functions {
        let signature = parsed.type_by_index(import.type_idx)?;
        import_functions.push(ImportFunction {
            module: import.module.clone(),
            name: import.name.clone(),
            params: signature.params.clone(),
            results: signature.results.clone(),
        });
    }
    import_functions.sort_by(|left, right| {
        left.module
            .cmp(&right.module)
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut exports: Vec<Export> = parsed
        .exports
        .iter()
        .map(|entry| Export {
            name: entry.name.clone(),
            kind: entry.kind,
            index: entry.index,
        })
        .collect();
    exports.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.index.cmp(&right.index))
    });

    let export_names_by_function_index = parsed.function_export_names_by_index();
    let imported_count = parsed.imported_functions.len();
    let mut function_bodies: Vec<FunctionBodySummary> =
        Vec::with_capacity(parsed.defined_bodies.len());
    for (defined_index, body) in parsed.defined_bodies.iter().enumerate() {
        let global_index = imported_count
            .checked_add(defined_index)
            .ok_or_else(|| internal_error("internal error: function index overflow"))?;
        let global_index_u32 = usize_to_u32(global_index)?;

        let symbol = export_names_by_function_index
            .get(&global_index_u32)
            .and_then(|names| names.first())
            .cloned()
            .unwrap_or_else(|| format!("func_{global_index}"));

        let signature = parsed.function_type_for_index(global_index)?;

        function_bodies.push(FunctionBodySummary {
            function_index: global_index_u32,
            symbol,
            params: signature.params.clone(),
            results: signature.results.clone(),
            opcodes: render_opcodes(&body.instructions),
            instructions: body.instructions.clone(),
        });
    }
    function_bodies.sort_by(|left, right| left.symbol.cmp(&right.symbol));

    Ok(DecodedModuleSummary {
        import_functions,
        exports,
        defined_function_bodies: parsed.defined_bodies.len(),
        function_bodies,
    })
}

/// Build an approximate control-flow graph sketch for a single exported function.
///
/// **Important:** This is a template-based approximation, not a real CFG
/// reconstruction algorithm. It detects the presence of `Loop` or `If`/`Else`
/// instructions and returns a fixed template graph. It does not handle nested
/// control flow, multiple loops, `br_table`, or mixed constructs correctly.
/// The output is suitable for high-level structural annotation (e.g. the
/// `explain` CLI command) but must not be relied upon for analysis correctness.
pub fn build_cfg_summary(wasm: &[u8], export: &str) -> CoreResult<CfgSummary> {
    build_cfg_summary_with_limits(wasm, export, &ParseLimits::default())
}

/// Template-based CFG sketch. See [`build_cfg_summary`] for limitations.
pub fn build_cfg_summary_with_limits(
    wasm: &[u8],
    export: &str,
    limits: &ParseLimits,
) -> CoreResult<CfgSummary> {
    validate_wasm_with_limits(wasm, limits)?;
    validate_export_name(export)?;
    let parsed = parse_module(wasm, limits)?;
    let function = parsed.defined_function_for_export(export)?;

    if function
        .instructions
        .iter()
        .any(|op| matches!(op, Instruction::Loop))
    {
        let blocks = vec![
            "b0_entry".to_owned(),
            "b1_loop_header".to_owned(),
            "b2_loop_body".to_owned(),
            "b3_loop_exit".to_owned(),
            "b4_exit".to_owned(),
        ];
        let edges = vec![
            CfgEdge {
                from: "b0_entry".to_owned(),
                to: "b1_loop_header".to_owned(),
                kind: EdgeKind::Fallthrough,
            },
            CfgEdge {
                from: "b1_loop_header".to_owned(),
                to: "b2_loop_body".to_owned(),
                kind: EdgeKind::Fallthrough,
            },
            CfgEdge {
                from: "b2_loop_body".to_owned(),
                to: "b3_loop_exit".to_owned(),
                kind: EdgeKind::BranchTrue,
            },
            CfgEdge {
                from: "b2_loop_body".to_owned(),
                to: "b1_loop_header".to_owned(),
                kind: EdgeKind::BackEdge,
            },
            CfgEdge {
                from: "b3_loop_exit".to_owned(),
                to: "b4_exit".to_owned(),
                kind: EdgeKind::Unconditional,
            },
        ];

        return Ok(CfgSummary {
            entry: "b0_entry".to_owned(),
            blocks,
            edges,
            exits: vec!["b4_exit".to_owned()],
        });
    }

    let has_if = function
        .instructions
        .iter()
        .any(|op| matches!(op, Instruction::If));
    let has_else = function
        .instructions
        .iter()
        .any(|op| matches!(op, Instruction::Else));

    if has_if && has_else {
        let blocks = vec![
            "b0_entry".to_owned(),
            "b1_if_true".to_owned(),
            "b2_if_false".to_owned(),
            "b3_merge".to_owned(),
            "b4_exit".to_owned(),
        ];
        let edges = vec![
            CfgEdge {
                from: "b0_entry".to_owned(),
                to: "b1_if_true".to_owned(),
                kind: EdgeKind::BranchTrue,
            },
            CfgEdge {
                from: "b0_entry".to_owned(),
                to: "b2_if_false".to_owned(),
                kind: EdgeKind::BranchFalse,
            },
            CfgEdge {
                from: "b1_if_true".to_owned(),
                to: "b3_merge".to_owned(),
                kind: EdgeKind::Unconditional,
            },
            CfgEdge {
                from: "b2_if_false".to_owned(),
                to: "b3_merge".to_owned(),
                kind: EdgeKind::Unconditional,
            },
            CfgEdge {
                from: "b3_merge".to_owned(),
                to: "b4_exit".to_owned(),
                kind: EdgeKind::Fallthrough,
            },
        ];

        return Ok(CfgSummary {
            entry: "b0_entry".to_owned(),
            blocks,
            edges,
            exits: vec!["b4_exit".to_owned()],
        });
    }

    Ok(CfgSummary {
        entry: "b0_entry".to_owned(),
        blocks: vec!["b0_entry".to_owned(), "b1_exit".to_owned()],
        edges: vec![CfgEdge {
            from: "b0_entry".to_owned(),
            to: "b1_exit".to_owned(),
            kind: EdgeKind::Fallthrough,
        }],
        exits: vec!["b1_exit".to_owned()],
    })
}

/// Build an approximate SSA-like instruction summary for a single exported function.
///
/// **Important:** This is a template-based approximation, not a real SSA lifting
/// pass. It filters instructions through a string-rendering function and heuristically
/// sets `phi_nodes` to 0 or 1 based on the presence of `If`/`Else`. It does not
/// perform def-use analysis, value numbering, or actual phi-node insertion.
/// The output is suitable for high-level annotation (e.g. the `explain` CLI
/// command) but must not be relied upon for analysis correctness.
pub fn lift_function_to_ssa_summary(wasm: &[u8], export: &str) -> CoreResult<SsaSummary> {
    lift_function_to_ssa_summary_with_limits(wasm, export, &ParseLimits::default())
}

/// Template-based SSA sketch. See [`lift_function_to_ssa_summary`] for limitations.
pub fn lift_function_to_ssa_summary_with_limits(
    wasm: &[u8],
    export: &str,
    limits: &ParseLimits,
) -> CoreResult<SsaSummary> {
    validate_wasm_with_limits(wasm, limits)?;
    validate_export_name(export)?;

    let parsed = parse_module(wasm, limits)?;
    let (function_index, function) = parsed.defined_function_for_export_with_index(export)?;

    if function
        .instructions
        .iter()
        .any(|op| matches!(op, Instruction::CallIndirect { .. }))
    {
        return Err(unsupported_error(
            "unsupported construct: call_indirect is not supported in SSA lifting",
        ));
    }

    let signature = parsed.function_type_for_index(function_index)?;
    let params = (0..signature.params.len())
        .map(|idx| format!("p{idx}"))
        .collect();

    let instructions = function
        .instructions
        .iter()
        .filter_map(instruction_to_ssa_instruction)
        .map(ToOwned::to_owned)
        .collect();

    let has_if = function
        .instructions
        .iter()
        .any(|op| matches!(op, Instruction::If));
    let has_else = function
        .instructions
        .iter()
        .any(|op| matches!(op, Instruction::Else));

    Ok(SsaSummary {
        params,
        instructions,
        phi_nodes: usize::from(has_if && has_else),
        terminator: "return".to_owned(),
    })
}

pub fn resolve_soroban_imports(wasm: &[u8]) -> CoreResult<Vec<SorobanImportResolution>> {
    resolve_soroban_imports_with_limits(wasm, &ParseLimits::default())
}

pub fn resolve_soroban_imports_with_limits(
    wasm: &[u8],
    limits: &ParseLimits,
) -> CoreResult<Vec<SorobanImportResolution>> {
    validate_wasm_with_limits(wasm, limits)?;
    let parsed = parse_module(wasm, limits)?;

    let resolved = resolve_knowledge_imports(
        parsed
            .imported_functions
            .iter()
            .map(|import| (import.module.clone(), import.name.clone())),
    )
    .into_iter()
    .map(|resolved| SorobanImportResolution {
        module: resolved.module,
        name: resolved.name,
        kind: match resolved.kind {
            SorobanSymbolKind::EnvBuiltin => SorobanImportKind::EnvBuiltin,
            SorobanSymbolKind::EnvUnknown => SorobanImportKind::EnvUnknown,
            SorobanSymbolKind::NonEnv => SorobanImportKind::NonEnv,
        },
    })
    .collect();

    Ok(resolved)
}

/// Extract custom sections from a WASM module, grouped by section name.
///
/// This is a building block for Soroban-aware reconstruction (e.g. the
/// `contractspecv0` / `contractmetav0` / `contractenvmetav0` sections).
pub fn extract_custom_sections_by_name(wasm: &[u8]) -> CoreResult<BTreeMap<String, Vec<Vec<u8>>>> {
    extract_custom_sections_by_name_with_limits(wasm, &ParseLimits::default())
}

pub fn extract_custom_sections_by_name_with_limits(
    wasm: &[u8],
    limits: &ParseLimits,
) -> CoreResult<BTreeMap<String, Vec<Vec<u8>>>> {
    validate_wasm_with_limits(wasm, limits)?;

    let mut sections: BTreeMap<String, Vec<Vec<u8>>> = BTreeMap::new();
    for payload in Parser::new(0).parse_all(wasm) {
        let payload = payload.map_err(|error| {
            malformed_error(format!("malformed wasm: wasmparser failed: {error}"))
        })?;

        let Payload::CustomSection(reader) = payload else {
            continue;
        };

        sections
            .entry(reader.name().to_string())
            .or_default()
            .push(reader.data().to_vec());
    }

    Ok(sections)
}

/// Decode Soroban custom sections into a typed semantic model.
///
/// The decoder is deterministic and strict for malformed payloads. Unknown
/// lines are ignored to allow forward-compatible section growth.
pub fn decode_soroban_custom_sections(wasm: &[u8]) -> CoreResult<SorobanCustomSections> {
    decode_soroban_custom_sections_with_limits(wasm, &ParseLimits::default())
}

pub fn decode_soroban_custom_sections_with_limits(
    wasm: &[u8],
    limits: &ParseLimits,
) -> CoreResult<SorobanCustomSections> {
    let sections = extract_custom_sections_by_name_with_limits(wasm, limits)?;

    let mut contract_spec: Option<SorobanContractSpec> = None;
    if let Some(payloads) = sections.get("contractspecv0") {
        let mut merged = SorobanContractSpec {
            functions: Vec::new(),
            types: Vec::new(),
            errors: Vec::new(),
        };

        for payload in payloads {
            let decoded = decode_contractspec_payload(payload)?;
            merged.functions.extend(decoded.functions);
            merged.types.extend(decoded.types);
            merged.errors.extend(decoded.errors);
        }
        merged
            .functions
            .sort_by(|left, right| left.name.cmp(&right.name));
        merged
            .types
            .sort_by(|left, right| left.name.cmp(&right.name));
        merged.errors.sort_by(|left, right| {
            left.code
                .cmp(&right.code)
                .then_with(|| left.name.cmp(&right.name))
        });
        contract_spec = Some(merged);
    }

    let contract_meta = match sections.get("contractmetav0") {
        Some(payloads) => {
            let mut merged = SorobanContractMeta {
                contract_name: None,
                version: None,
                entries: Vec::new(),
            };
            for payload in payloads {
                let decoded = decode_contractmeta_payload(payload)?;
                // First-wins strategy: when multiple contractmetav0 payloads
                // provide different contract_name or version values, the first
                // non-None value wins. This is intentional for deterministic
                // merging of forward-compatible section growth.
                if merged.contract_name.is_none() {
                    merged.contract_name = decoded.contract_name;
                }
                if merged.version.is_none() {
                    merged.version = decoded.version;
                }
                merged.entries.extend(decoded.entries);
            }
            merged.entries.sort_by(|left, right| {
                left.key
                    .cmp(&right.key)
                    .then_with(|| left.value.cmp(&right.value))
            });
            Some(merged)
        }
        None => None,
    };

    let contract_env_meta = match sections.get("contractenvmetav0") {
        Some(payloads) => {
            let mut selected: Option<SorobanContractEnvMeta> = None;
            for payload in payloads {
                let decoded = decode_contractenvmeta_payload(payload)?;
                selected = match selected {
                    None => Some(decoded),
                    Some(existing) => Some(select_env_meta(existing, decoded)),
                };
            }
            selected
        }
        None => None,
    };

    Ok(SorobanCustomSections {
        contract_spec,
        contract_meta,
        contract_env_meta,
    })
}

fn decode_contractspec_payload(payload: &[u8]) -> CoreResult<SorobanContractSpec> {
    let text = payload_to_text(payload, "contractspecv0")?;
    let mut functions = Vec::new();
    let mut types = Vec::new();
    let mut errors = Vec::new();

    for line in semantic_lines(text) {
        let mut parts = line.split('|');
        let tag = parts.next().unwrap_or_default();

        match tag {
            "fn" => {
                let name = required_part(parts.next(), "contractspecv0", "function name")?;
                let input_spec = parts.next().ok_or_else(|| {
                    malformed_error("malformed contractspecv0 decode: missing function inputs")
                })?;
                let output_spec = parts.next().ok_or_else(|| {
                    malformed_error("malformed contractspecv0 decode: missing function output")
                })?;
                let doc = parts
                    .next()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                let inputs = parse_named_type_list(input_spec, "contractspecv0 function inputs")?;
                let output = if output_spec.trim().is_empty() {
                    None
                } else {
                    Some(output_spec.trim().to_string())
                };
                functions.push(SorobanSpecFunction {
                    name: name.trim().to_string(),
                    inputs,
                    output,
                    doc,
                });
            }
            "type" => {
                let kind_raw = required_part(parts.next(), "contractspecv0", "type kind")?;
                let name = required_part(parts.next(), "contractspecv0", "type name")?;
                let payload = required_part(parts.next(), "contractspecv0", "type payload")?;

                let kind = match kind_raw.trim() {
                    "struct" => SorobanSpecTypeKind::Struct,
                    "enum" => SorobanSpecTypeKind::Enum,
                    "alias" => SorobanSpecTypeKind::Alias,
                    other => {
                        return Err(malformed_error(format!(
                            "malformed contractspecv0 decode: unknown type kind `{other}`"
                        )));
                    }
                };

                let (fields, variants, alias_target) = match kind {
                    SorobanSpecTypeKind::Struct => (
                        parse_named_type_list(payload, "contractspecv0 struct fields")?,
                        Vec::new(),
                        None,
                    ),
                    SorobanSpecTypeKind::Enum => (
                        Vec::new(),
                        payload
                            .split(',')
                            .map(str::trim)
                            .filter(|entry| !entry.is_empty())
                            .map(ToOwned::to_owned)
                            .collect(),
                        None,
                    ),
                    SorobanSpecTypeKind::Alias => {
                        (Vec::new(), Vec::new(), Some(payload.trim().to_string()))
                    }
                };

                types.push(SorobanSpecType {
                    name: name.trim().to_string(),
                    kind,
                    fields,
                    variants,
                    alias_target,
                });
            }
            "error" => {
                let name = required_part(parts.next(), "contractspecv0", "error name")?;
                let code_text = required_part(parts.next(), "contractspecv0", "error code")?;
                let code = code_text.trim().parse::<i32>().map_err(|_| {
                    malformed_error(format!(
                        "malformed contractspecv0 decode: invalid error code `{code_text}`"
                    ))
                })?;
                let doc = parts
                    .next()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                errors.push(SorobanSpecError {
                    name: name.trim().to_string(),
                    code,
                    doc,
                });
            }
            _ => {}
        }
    }

    Ok(SorobanContractSpec {
        functions,
        types,
        errors,
    })
}

fn decode_contractmeta_payload(payload: &[u8]) -> CoreResult<SorobanContractMeta> {
    let text = payload_to_text(payload, "contractmetav0")?;
    let mut contract_name = None;
    let mut version = None;
    let mut entries = Vec::new();

    for line in semantic_lines(text) {
        if let Some(rest) = line.strip_prefix("contract_name=") {
            let value = rest.trim();
            if !value.is_empty() {
                contract_name = Some(value.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("version=") {
            let value = rest.trim();
            if !value.is_empty() {
                version = Some(value.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("entry|") {
            let mut parts = rest.splitn(2, '|');
            let key = parts.next().unwrap_or_default().trim();
            let value = parts.next().unwrap_or_default().trim();
            if key.is_empty() {
                return Err(malformed_error(
                    "malformed contractmetav0 decode: entry key must not be empty",
                ));
            }
            entries.push(SorobanMetaEntry {
                key: key.to_string(),
                value: value.to_string(),
            });
        }
    }

    Ok(SorobanContractMeta {
        contract_name,
        version,
        entries,
    })
}

fn decode_contractenvmeta_payload(payload: &[u8]) -> CoreResult<SorobanContractEnvMeta> {
    let text = payload_to_text(payload, "contractenvmetav0")?;
    let mut protocol = None;
    let mut interface_version = None;
    let mut sdk_version = None;

    for line in semantic_lines(text) {
        if let Some(rest) = line.strip_prefix("protocol=") {
            protocol = Some(rest.trim().parse::<u32>().map_err(|_| {
                malformed_error(format!(
                    "malformed contractenvmetav0 decode: invalid protocol `{}`",
                    rest.trim()
                ))
            })?);
            continue;
        }
        if let Some(rest) = line.strip_prefix("interface_version=") {
            interface_version = Some(rest.trim().parse::<u32>().map_err(|_| {
                malformed_error(format!(
                    "malformed contractenvmetav0 decode: invalid interface_version `{}`",
                    rest.trim()
                ))
            })?);
            continue;
        }
        if let Some(rest) = line.strip_prefix("sdk_version=") {
            let value = rest.trim();
            if !value.is_empty() {
                sdk_version = Some(value.to_string());
            }
        }
    }

    let protocol = protocol.ok_or_else(|| {
        malformed_error("malformed contractenvmetav0 decode: missing required `protocol` value")
    })?;

    Ok(SorobanContractEnvMeta {
        protocol,
        interface_version,
        sdk_version,
    })
}

/// Select the env meta entry with the higher protocol version.
/// Tie-breaking: when both have equal protocol versions, the first entry wins.
/// This is a deterministic, stable strategy that ensures consistent output
/// regardless of payload ordering in the binary.
fn select_env_meta(
    first: SorobanContractEnvMeta,
    second: SorobanContractEnvMeta,
) -> SorobanContractEnvMeta {
    if second.protocol > first.protocol {
        second
    } else {
        first
    }
}

fn payload_to_text<'a>(payload: &'a [u8], section: &str) -> CoreResult<&'a str> {
    std::str::from_utf8(payload).map_err(|_| {
        malformed_error(format!(
            "malformed {section} decode: payload is not valid UTF-8"
        ))
    })
}

fn semantic_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

fn required_part<'a>(value: Option<&'a str>, section: &str, field: &str) -> CoreResult<&'a str> {
    let value = value.unwrap_or_default().trim();
    if value.is_empty() {
        return Err(malformed_error(format!(
            "malformed {section} decode: missing {field}"
        )));
    }
    Ok(value)
}

fn parse_named_type_list(text: &str, context: &str) -> CoreResult<Vec<SorobanSpecField>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut fields = Vec::new();
    for entry in text.split(',') {
        let mut parts = entry.splitn(2, ':');
        let name = parts.next().unwrap_or_default().trim();
        let ty = parts.next().unwrap_or_default().trim();
        if name.is_empty() || ty.is_empty() {
            return Err(malformed_error(format!(
                "malformed {context}: expected `name:type` entries"
            )));
        }
        fields.push(SorobanSpecField {
            name: name.to_string(),
            ty: ty.to_string(),
        });
    }

    Ok(fields)
}

pub fn summarize_export_opcodes(wasm: &[u8], export: &str) -> CoreResult<Vec<String>> {
    summarize_export_opcodes_with_limits(wasm, export, &ParseLimits::default())
}

pub fn summarize_export_opcodes_with_limits(
    wasm: &[u8],
    export: &str,
    limits: &ParseLimits,
) -> CoreResult<Vec<String>> {
    validate_wasm_with_limits(wasm, limits)?;
    validate_export_name(export)?;

    let parsed = parse_module(wasm, limits)?;
    let function = parsed.defined_function_for_export(export)?;
    Ok(render_opcodes(&function.instructions))
}

fn validate_wasm_header(wasm: &[u8]) -> CoreResult<()> {
    if wasm.len() < 8 {
        return Err(malformed_error(
            "malformed wasm: binary shorter than magic/version header",
        ));
    }

    let magic = &wasm[0..4];
    let version = &wasm[4..8];

    if magic != [0x00, 0x61, 0x73, 0x6d] || version != [0x01, 0x00, 0x00, 0x00] {
        return Err(malformed_error(
            "malformed wasm: invalid magic or version header",
        ));
    }

    Ok(())
}

fn validate_wasm_with_limits(wasm: &[u8], limits: &ParseLimits) -> CoreResult<()> {
    validate_wasm_header(wasm)?;
    if wasm.len() > limits.max_wasm_bytes {
        return Err(resource_limit_error(format!(
            "resource limit exceeded: max_wasm_bytes={} actual={}",
            limits.max_wasm_bytes,
            wasm.len()
        )));
    }
    Ok(())
}

fn validate_export_name(export: &str) -> CoreResult<()> {
    if export.trim().is_empty() {
        return Err(unsupported_error(
            "unsupported request: export name must not be empty",
        ));
    }
    Ok(())
}

fn malformed_error(message: impl Into<String>) -> CoreError {
    CoreError {
        kind: CoreErrorKind::MalformedBinary,
        message: message.into(),
    }
}

fn unsupported_error(message: impl Into<String>) -> CoreError {
    CoreError {
        kind: CoreErrorKind::UnsupportedConstruct,
        message: message.into(),
    }
}

fn resource_limit_error(message: impl Into<String>) -> CoreError {
    CoreError {
        kind: CoreErrorKind::ResourceLimitExceeded,
        message: message.into(),
    }
}

fn internal_error(message: impl Into<String>) -> CoreError {
    CoreError {
        kind: CoreErrorKind::Internal,
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionType {
    params: Vec<String>,
    results: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportFunctionRaw {
    module: String,
    name: String,
    type_idx: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportEntry {
    name: String,
    kind: ExportKind,
    index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedFunctionBody {
    instructions: Vec<Instruction>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ParsedModule {
    types: Vec<FunctionType>,
    imported_functions: Vec<ImportFunctionRaw>,
    defined_function_type_indices: Vec<u32>,
    exports: Vec<ExportEntry>,
    defined_bodies: Vec<DecodedFunctionBody>,
}

impl ParsedModule {
    fn type_by_index(&self, index: u32) -> CoreResult<&FunctionType> {
        let idx = u32_to_usize(index)?;
        self.types
            .get(idx)
            .ok_or_else(|| malformed_error(format!("malformed wasm: missing type index {index}")))
    }

    fn function_type_for_index(&self, function_index: usize) -> CoreResult<&FunctionType> {
        let imported_count = self.imported_functions.len();

        let type_idx = if function_index < imported_count {
            self.imported_functions
                .get(function_index)
                .ok_or_else(|| {
                    malformed_error(format!(
                        "malformed wasm: function index {function_index} out of range"
                    ))
                })?
                .type_idx
        } else {
            let defined_index = function_index - imported_count;
            *self
                .defined_function_type_indices
                .get(defined_index)
                .ok_or_else(|| {
                    malformed_error(format!(
                        "malformed wasm: function index {function_index} out of range"
                    ))
                })?
        };

        self.type_by_index(type_idx)
    }

    fn function_export_names_by_index(&self) -> BTreeMap<u32, Vec<String>> {
        let mut names_by_index: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for export in &self.exports {
            if export.kind == ExportKind::Function {
                names_by_index
                    .entry(export.index)
                    .or_default()
                    .push(export.name.clone());
            }
        }

        for names in names_by_index.values_mut() {
            names.sort();
        }

        names_by_index
    }

    fn defined_function_for_export(&self, export: &str) -> CoreResult<&DecodedFunctionBody> {
        self.defined_function_for_export_with_index(export)
            .map(|(_, function)| function)
    }

    fn defined_function_for_export_with_index(
        &self,
        export: &str,
    ) -> CoreResult<(usize, &DecodedFunctionBody)> {
        let export_entry = self
            .exports
            .iter()
            .find(|entry| entry.name == export)
            .ok_or_else(|| {
                unsupported_error(format!(
                    "unsupported request: export '{export}' was not found"
                ))
            })?;

        if export_entry.kind != ExportKind::Function {
            return Err(unsupported_error(format!(
                "unsupported request: export '{export}' is not a function"
            )));
        }

        let function_index = u32_to_usize(export_entry.index)?;
        let imported_count = self.imported_functions.len();
        if function_index < imported_count {
            return Err(unsupported_error(format!(
                "unsupported request: export '{export}' refers to an imported function",
            )));
        }

        let defined_index = function_index - imported_count;
        let function = self.defined_bodies.get(defined_index).ok_or_else(|| {
            malformed_error(format!(
                "malformed wasm: export '{export}' function index is out of bounds",
            ))
        })?;

        Ok((function_index, function))
    }
}

fn parse_module(wasm: &[u8], limits: &ParseLimits) -> CoreResult<ParsedModule> {
    let mut parsed = ParsedModule::default();
    let mut offset = 8_usize;
    let mut seen_non_custom_sections = [false; 13];

    while offset < wasm.len() {
        let section_id = read_u8(wasm, &mut offset, "section id")?;
        let section_len_u32 = read_var_u32(wasm, &mut offset, "section length")?;
        let section_len = u32_to_usize(section_len_u32)?;
        let section_end = offset
            .checked_add(section_len)
            .ok_or_else(|| malformed_error("malformed wasm: section length overflow"))?;
        if section_end > wasm.len() {
            return Err(malformed_error(
                "malformed wasm: section payload extends past end of binary",
            ));
        }

        if section_id != 0 {
            let section_index = usize::from(section_id);
            if section_index >= seen_non_custom_sections.len() {
                return Err(unsupported_error(format!(
                    "unsupported construct: section id {section_id} is not supported"
                )));
            }
            if seen_non_custom_sections[section_index] {
                return Err(malformed_error(format!(
                    "malformed wasm: duplicate section id {section_id}",
                )));
            }
            seen_non_custom_sections[section_index] = true;
        }

        let payload = &wasm[offset..section_end];
        match section_id {
            0 => {}
            1 => parse_type_section(payload, &mut parsed.types)?,
            2 => parse_import_section(payload, &mut parsed.imported_functions)?,
            3 => parse_function_section(payload, &mut parsed.defined_function_type_indices)?,
            7 => parse_export_section(payload, &mut parsed.exports)?,
            10 => parse_code_section(payload, &mut parsed.defined_bodies, limits)?,
            4 | 5 | 6 | 8 | 9 | 11 | 12 => {}
            _ => {
                return Err(unsupported_error(format!(
                    "unsupported construct: section id {section_id} is not supported"
                )));
            }
        }

        offset = section_end;
    }

    for imported in &parsed.imported_functions {
        parsed.type_by_index(imported.type_idx)?;
    }
    for type_index in &parsed.defined_function_type_indices {
        parsed.type_by_index(*type_index)?;
    }

    if parsed.defined_function_type_indices.len() != parsed.defined_bodies.len() {
        return Err(malformed_error(
            "malformed wasm: function and code section function count mismatch",
        ));
    }

    Ok(parsed)
}

fn parse_type_section(bytes: &[u8], out: &mut Vec<FunctionType>) -> CoreResult<()> {
    let mut offset = 0_usize;
    let count_u32 = read_var_u32(bytes, &mut offset, "type section count")?;
    let count = u32_to_usize(count_u32)?;

    for _ in 0..count {
        let form = read_u8(bytes, &mut offset, "type form")?;
        if form != 0x60 {
            return Err(unsupported_error(
                "unsupported construct: only function type entries are supported",
            ));
        }

        let params = read_value_type_vec(bytes, &mut offset, "type params")?;
        let results = read_value_type_vec(bytes, &mut offset, "type results")?;
        out.push(FunctionType { params, results });
    }

    ensure_consumed(bytes, offset, "type section")
}

fn parse_import_section(bytes: &[u8], out: &mut Vec<ImportFunctionRaw>) -> CoreResult<()> {
    let mut offset = 0_usize;
    let count_u32 = read_var_u32(bytes, &mut offset, "import section count")?;
    let count = u32_to_usize(count_u32)?;

    for _ in 0..count {
        let module = read_name(bytes, &mut offset, "import module")?;
        let name = read_name(bytes, &mut offset, "import name")?;
        let descriptor = read_u8(bytes, &mut offset, "import descriptor")?;

        match descriptor {
            0x00 => {
                let type_idx = read_var_u32(bytes, &mut offset, "import function type index")?;
                out.push(ImportFunctionRaw {
                    module,
                    name,
                    type_idx,
                });
            }
            0x01 => skip_table_type(bytes, &mut offset)?,
            0x02 => skip_memory_type(bytes, &mut offset)?,
            0x03 => skip_global_type(bytes, &mut offset)?,
            0x04 => skip_tag_type(bytes, &mut offset)?,
            _ => {
                return Err(unsupported_error(format!(
                    "unsupported construct: import descriptor {descriptor} is not supported"
                )));
            }
        }
    }

    ensure_consumed(bytes, offset, "import section")
}

fn parse_function_section(bytes: &[u8], out: &mut Vec<u32>) -> CoreResult<()> {
    let mut offset = 0_usize;
    let count_u32 = read_var_u32(bytes, &mut offset, "function section count")?;
    let count = u32_to_usize(count_u32)?;

    for _ in 0..count {
        out.push(read_var_u32(
            bytes,
            &mut offset,
            "function section type index",
        )?);
    }

    ensure_consumed(bytes, offset, "function section")
}

fn parse_export_section(bytes: &[u8], out: &mut Vec<ExportEntry>) -> CoreResult<()> {
    let mut offset = 0_usize;
    let count_u32 = read_var_u32(bytes, &mut offset, "export section count")?;
    let count = u32_to_usize(count_u32)?;

    for _ in 0..count {
        let name = read_name(bytes, &mut offset, "export name")?;
        let kind_byte = read_u8(bytes, &mut offset, "export kind")?;
        let index = read_var_u32(bytes, &mut offset, "export index")?;

        let kind = match kind_byte {
            0x00 => ExportKind::Function,
            0x01 => ExportKind::Table,
            0x02 => ExportKind::Memory,
            0x03 => ExportKind::Global,
            _ => {
                return Err(unsupported_error(format!(
                    "unsupported construct: export kind {kind_byte} is not supported"
                )));
            }
        };

        out.push(ExportEntry { name, kind, index });
    }

    ensure_consumed(bytes, offset, "export section")
}

fn parse_code_section(
    bytes: &[u8],
    out: &mut Vec<DecodedFunctionBody>,
    limits: &ParseLimits,
) -> CoreResult<()> {
    let mut offset = 0_usize;
    let count_u32 = read_var_u32(bytes, &mut offset, "code section function count")?;
    let count = u32_to_usize(count_u32)?;

    for _ in 0..count {
        let body_size_u32 = read_var_u32(bytes, &mut offset, "function body size")?;
        let body_size = u32_to_usize(body_size_u32)?;
        let body_end = offset
            .checked_add(body_size)
            .ok_or_else(|| malformed_error("malformed wasm: function body length overflow"))?;
        if body_end > bytes.len() {
            return Err(malformed_error(
                "malformed wasm: function body extends past code section",
            ));
        }

        let body = parse_function_body(&bytes[offset..body_end], limits)?;
        out.push(body);
        offset = body_end;
    }

    ensure_consumed(bytes, offset, "code section")
}

fn parse_function_body(bytes: &[u8], limits: &ParseLimits) -> CoreResult<DecodedFunctionBody> {
    enum ControlFrame {
        Block,
        Loop,
        If { seen_else: bool },
    }

    let mut offset = 0_usize;
    let local_decl_count_u32 = read_var_u32(bytes, &mut offset, "local declaration count")?;
    let local_decl_count = u32_to_usize(local_decl_count_u32)?;

    for _ in 0..local_decl_count {
        let _local_count = read_var_u32(bytes, &mut offset, "local declaration entry count")?;
        let local_type = read_u8(bytes, &mut offset, "local declaration value type")?;
        value_type_name(local_type)?;
    }

    let mut instructions = Vec::new();
    let mut control_stack: Vec<ControlFrame> = Vec::new();
    let mut reached_function_end = false;

    while offset < bytes.len() {
        let opcode = read_u8(bytes, &mut offset, "opcode")?;
        let instruction = match opcode {
            0x02 => {
                read_block_type(bytes, &mut offset)?;
                control_stack.push(ControlFrame::Block);
                Instruction::Block
            }
            0x03 => {
                read_block_type(bytes, &mut offset)?;
                control_stack.push(ControlFrame::Loop);
                Instruction::Loop
            }
            0x04 => {
                read_block_type(bytes, &mut offset)?;
                control_stack.push(ControlFrame::If { seen_else: false });
                Instruction::If
            }
            0x05 => match control_stack.last_mut() {
                Some(ControlFrame::If { seen_else }) if !*seen_else => {
                    *seen_else = true;
                    Instruction::Else
                }
                Some(ControlFrame::If { .. }) => {
                    return Err(malformed_error(
                        "malformed wasm: duplicate else for active if block",
                    ));
                }
                _ => {
                    return Err(malformed_error(
                        "malformed wasm: else opcode without active if block",
                    ));
                }
            },
            0x0b => {
                if control_stack.is_empty() {
                    reached_function_end = true;
                    Instruction::End
                } else {
                    control_stack.pop();
                    Instruction::End
                }
            }
            0x0c => {
                let depth = read_var_u32(bytes, &mut offset, "br label")?;
                Instruction::Br { depth }
            }
            0x0d => {
                let depth = read_var_u32(bytes, &mut offset, "br_if label")?;
                Instruction::BrIf { depth }
            }
            0x0f => Instruction::Return,
            0x10 => {
                let function_index = read_var_u32(bytes, &mut offset, "call function index")?;
                Instruction::Call { function_index }
            }
            0x11 => {
                let type_index = read_var_u32(bytes, &mut offset, "call_indirect type index")?;
                let table_index = read_var_u32(bytes, &mut offset, "call_indirect table index")?;
                Instruction::CallIndirect {
                    type_index,
                    table_index,
                }
            }
            0x1a => Instruction::Drop,
            0x20 => {
                let local_index = read_var_u32(bytes, &mut offset, "local.get local index")?;
                Instruction::LocalGet { local_index }
            }
            0x21 => {
                let local_index = read_var_u32(bytes, &mut offset, "local.set local index")?;
                Instruction::LocalSet { local_index }
            }
            0x22 => {
                let local_index = read_var_u32(bytes, &mut offset, "local.tee local index")?;
                Instruction::LocalTee { local_index }
            }
            0x23 => {
                let global_index = read_var_u32(bytes, &mut offset, "global.get global index")?;
                Instruction::GlobalGet { global_index }
            }
            0x24 => {
                let global_index = read_var_u32(bytes, &mut offset, "global.set global index")?;
                Instruction::GlobalSet { global_index }
            }
            0x41 => {
                let value = read_var_i32(bytes, &mut offset, "i32.const literal")?;
                Instruction::I32Const { value }
            }
            0x42 => {
                let value = read_var_i64(bytes, &mut offset, "i64.const literal")?;
                Instruction::I64Const { value }
            }
            0x45 => Instruction::I32Eqz,
            0x46 => Instruction::I32Eq,
            0x47 => Instruction::I32Ne,
            0x48 => Instruction::I32LtS,
            0x49 => Instruction::I32LtU,
            0x4a => Instruction::I32GtS,
            0x4b => Instruction::I32GtU,
            0x4c => Instruction::I32LeS,
            0x4d => Instruction::I32LeU,
            0x4e => Instruction::I32GeS,
            0x4f => Instruction::I32GeU,
            0x50 => Instruction::I64Eqz,
            0x51 => Instruction::I64Eq,
            0x52 => Instruction::I64Ne,
            0x53 => Instruction::I64LtS,
            0x54 => Instruction::I64LtU,
            0x55 => Instruction::I64GtS,
            0x56 => Instruction::I64GtU,
            0x57 => Instruction::I64LeS,
            0x58 => Instruction::I64LeU,
            0x59 => Instruction::I64GeS,
            0x5a => Instruction::I64GeU,
            0x6a => Instruction::I32Add,
            0x6b => Instruction::I32Sub,
            0x6c => Instruction::I32Mul,
            0x6d => Instruction::I32DivS,
            0x6e => Instruction::I32DivU,
            0x6f => Instruction::I32RemS,
            0x70 => Instruction::I32RemU,
            0x71 => Instruction::I32And,
            0x72 => Instruction::I32Or,
            0x73 => Instruction::I32Xor,
            0x74 => Instruction::I32Shl,
            0x75 => Instruction::I32ShrS,
            0x76 => Instruction::I32ShrU,
            0x7c => Instruction::I64Add,
            0x7d => Instruction::I64Sub,
            0x7e => Instruction::I64Mul,
            0x7f => Instruction::I64DivS,
            0x80 => Instruction::I64DivU,
            0x81 => Instruction::I64RemS,
            0x82 => Instruction::I64RemU,
            0x83 => Instruction::I64And,
            0x84 => Instruction::I64Or,
            0x85 => Instruction::I64Xor,
            0x86 => Instruction::I64Shl,
            0x87 => Instruction::I64ShrS,
            0x88 => Instruction::I64ShrU,
            0x1b => Instruction::Select,
            0x0e => {
                let target_count = read_var_u32(bytes, &mut offset, "br_table target count")?;
                let target_count = u32_to_usize(target_count)?;
                let mut targets = Vec::with_capacity(target_count);
                for _ in 0..target_count {
                    targets.push(read_var_u32(bytes, &mut offset, "br_table target label")?);
                }
                let default_target = read_var_u32(bytes, &mut offset, "br_table default label")?;
                Instruction::BrTable {
                    targets,
                    default_target,
                }
            }
            _ => {
                return Err(unsupported_error(format!(
                    "unsupported construct: opcode 0x{opcode:02x} is not supported"
                )));
            }
        };

        if instructions.len() >= limits.max_instructions_per_function {
            return Err(resource_limit_error(format!(
                "resource limit exceeded: max_instructions_per_function={} (function body)",
                limits.max_instructions_per_function
            )));
        }

        match instruction {
            Instruction::Block | Instruction::Loop | Instruction::If => {
                if control_stack.len() > limits.max_block_nesting_depth {
                    return Err(resource_limit_error(format!(
                        "resource limit exceeded: max_block_nesting_depth={} (function body)",
                        limits.max_block_nesting_depth
                    )));
                }
            }
            _ => {}
        }

        instructions.push(instruction);

        if reached_function_end {
            if offset != bytes.len() {
                return Err(malformed_error(
                    "malformed wasm: trailing opcodes after function end",
                ));
            }
            break;
        }
    }

    if !reached_function_end || !matches!(instructions.last(), Some(Instruction::End)) {
        return Err(malformed_error(
            "malformed wasm: function body is missing terminating end opcode",
        ));
    }

    Ok(DecodedFunctionBody { instructions })
}

fn instruction_name(instruction: &Instruction) -> &'static str {
    match instruction {
        Instruction::Block => "block",
        Instruction::Loop => "loop",
        Instruction::If => "if",
        Instruction::Else => "else",
        Instruction::End => "end",
        Instruction::Br { .. } => "br",
        Instruction::BrIf { .. } => "br_if",
        Instruction::Call { .. } => "call",
        Instruction::CallIndirect { .. } => "call_indirect",
        Instruction::LocalGet { .. } => "local.get",
        Instruction::LocalSet { .. } => "local.set",
        Instruction::LocalTee { .. } => "local.tee",
        Instruction::GlobalGet { .. } => "global.get",
        Instruction::GlobalSet { .. } => "global.set",
        Instruction::I32Const { .. } => "i32.const",
        Instruction::I64Const { .. } => "i64.const",
        Instruction::I32Eqz => "i32.eqz",
        Instruction::I32Eq => "i32.eq",
        Instruction::I32Ne => "i32.ne",
        Instruction::I32LtS => "i32.lt_s",
        Instruction::I32LtU => "i32.lt_u",
        Instruction::I32GtS => "i32.gt_s",
        Instruction::I32GtU => "i32.gt_u",
        Instruction::I32LeS => "i32.le_s",
        Instruction::I32LeU => "i32.le_u",
        Instruction::I32GeS => "i32.ge_s",
        Instruction::I32GeU => "i32.ge_u",
        Instruction::I64Eqz => "i64.eqz",
        Instruction::I64Eq => "i64.eq",
        Instruction::I64Ne => "i64.ne",
        Instruction::I64LtS => "i64.lt_s",
        Instruction::I64LtU => "i64.lt_u",
        Instruction::I64GtS => "i64.gt_s",
        Instruction::I64GtU => "i64.gt_u",
        Instruction::I64LeS => "i64.le_s",
        Instruction::I64LeU => "i64.le_u",
        Instruction::I64GeS => "i64.ge_s",
        Instruction::I64GeU => "i64.ge_u",
        Instruction::I32Add => "i32.add",
        Instruction::I32Sub => "i32.sub",
        Instruction::I32Mul => "i32.mul",
        Instruction::I32DivS => "i32.div_s",
        Instruction::I32DivU => "i32.div_u",
        Instruction::I32RemS => "i32.rem_s",
        Instruction::I32RemU => "i32.rem_u",
        Instruction::I32And => "i32.and",
        Instruction::I32Or => "i32.or",
        Instruction::I32Xor => "i32.xor",
        Instruction::I32Shl => "i32.shl",
        Instruction::I32ShrS => "i32.shr_s",
        Instruction::I32ShrU => "i32.shr_u",
        Instruction::I64Add => "i64.add",
        Instruction::I64Sub => "i64.sub",
        Instruction::I64Mul => "i64.mul",
        Instruction::I64DivS => "i64.div_s",
        Instruction::I64DivU => "i64.div_u",
        Instruction::I64RemS => "i64.rem_s",
        Instruction::I64RemU => "i64.rem_u",
        Instruction::I64And => "i64.and",
        Instruction::I64Or => "i64.or",
        Instruction::I64Xor => "i64.xor",
        Instruction::I64Shl => "i64.shl",
        Instruction::I64ShrS => "i64.shr_s",
        Instruction::I64ShrU => "i64.shr_u",
        Instruction::Select => "select",
        Instruction::BrTable { .. } => "br_table",
        Instruction::Drop => "drop",
        Instruction::Return => "return",
    }
}

fn instruction_to_ssa_instruction(instruction: &Instruction) -> Option<&'static str> {
    match instruction {
        Instruction::I32Add => Some("i32.add"),
        Instruction::I32Sub => Some("i32.sub"),
        Instruction::I32Mul => Some("i32.mul"),
        Instruction::I32DivS => Some("i32.div_s"),
        Instruction::I32DivU => Some("i32.div_u"),
        Instruction::I32RemS => Some("i32.rem_s"),
        Instruction::I32RemU => Some("i32.rem_u"),
        Instruction::I32And => Some("i32.and"),
        Instruction::I32Or => Some("i32.or"),
        Instruction::I32Xor => Some("i32.xor"),
        Instruction::I32Shl => Some("i32.shl"),
        Instruction::I32ShrS => Some("i32.shr_s"),
        Instruction::I32ShrU => Some("i32.shr_u"),
        Instruction::I64Add => Some("i64.add"),
        Instruction::I64Sub => Some("i64.sub"),
        Instruction::I64Mul => Some("i64.mul"),
        Instruction::I64DivS => Some("i64.div_s"),
        Instruction::I64DivU => Some("i64.div_u"),
        Instruction::I64RemS => Some("i64.rem_s"),
        Instruction::I64RemU => Some("i64.rem_u"),
        Instruction::I64And => Some("i64.and"),
        Instruction::I64Or => Some("i64.or"),
        Instruction::I64Xor => Some("i64.xor"),
        Instruction::I64Shl => Some("i64.shl"),
        Instruction::I64ShrS => Some("i64.shr_s"),
        Instruction::I64ShrU => Some("i64.shr_u"),
        Instruction::I32Eqz => Some("i32.eqz"),
        Instruction::I32Eq => Some("i32.eq"),
        Instruction::I32Ne => Some("i32.ne"),
        Instruction::I32LtS => Some("i32.lt_s"),
        Instruction::I32LtU => Some("i32.lt_u"),
        Instruction::I32GtS => Some("i32.gt_s"),
        Instruction::I32GtU => Some("i32.gt_u"),
        Instruction::I32LeS => Some("i32.le_s"),
        Instruction::I32LeU => Some("i32.le_u"),
        Instruction::I32GeS => Some("i32.ge_s"),
        Instruction::I32GeU => Some("i32.ge_u"),
        Instruction::I64Eqz => Some("i64.eqz"),
        Instruction::I64Eq => Some("i64.eq"),
        Instruction::I64Ne => Some("i64.ne"),
        Instruction::I64LtS => Some("i64.lt_s"),
        Instruction::I64LtU => Some("i64.lt_u"),
        Instruction::I64GtS => Some("i64.gt_s"),
        Instruction::I64GtU => Some("i64.gt_u"),
        Instruction::I64LeS => Some("i64.le_s"),
        Instruction::I64LeU => Some("i64.le_u"),
        Instruction::I64GeS => Some("i64.ge_s"),
        Instruction::I64GeU => Some("i64.ge_u"),
        Instruction::Call { .. } => Some("call"),
        Instruction::LocalGet { .. } => Some("local.get"),
        Instruction::LocalSet { .. } => Some("local.set"),
        Instruction::LocalTee { .. } => Some("local.tee"),
        Instruction::GlobalGet { .. } => Some("global.get"),
        Instruction::GlobalSet { .. } => Some("global.set"),
        Instruction::I32Const { .. } => Some("i32.const"),
        Instruction::I64Const { .. } => Some("i64.const"),
        Instruction::Drop => Some("drop"),
        Instruction::Return => Some("return"),
        Instruction::Block
        | Instruction::Loop
        | Instruction::If
        | Instruction::Else
        | Instruction::End
        | Instruction::Br { .. }
        | Instruction::BrIf { .. }
        | Instruction::CallIndirect { .. }
        | Instruction::Select
        | Instruction::BrTable { .. } => None,
    }
}

fn render_opcodes(instructions: &[Instruction]) -> Vec<String> {
    instructions
        .iter()
        .map(|op| instruction_name(op).to_owned())
        .collect()
}

fn read_u8(bytes: &[u8], offset: &mut usize, context: &str) -> CoreResult<u8> {
    let value = *bytes
        .get(*offset)
        .ok_or_else(|| malformed_error(format!("malformed wasm: missing {context}")))?;
    *offset += 1;
    Ok(value)
}

fn read_var_u32(bytes: &[u8], offset: &mut usize, context: &str) -> CoreResult<u32> {
    let mut result = 0_u32;
    let mut shift = 0_u32;

    loop {
        let byte = read_u8(bytes, offset, context)?;
        let low_bits = u32::from(byte & 0x7f);
        let shifted = low_bits.checked_shl(shift).ok_or_else(|| {
            malformed_error(format!("malformed wasm: invalid {context} encoding"))
        })?;
        result |= shifted;

        if (byte & 0x80) == 0 {
            return Ok(result);
        }

        shift += 7;
        if shift >= 35 {
            return Err(malformed_error(format!(
                "malformed wasm: invalid {context} encoding",
            )));
        }
    }
}

fn read_var_i32(bytes: &[u8], offset: &mut usize, context: &str) -> CoreResult<i32> {
    let mut result = 0_i64;
    let mut shift = 0_u32;
    let mut byte = read_u8(bytes, offset, context)?;

    loop {
        result |= i64::from(byte & 0x7f).checked_shl(shift).ok_or_else(|| {
            malformed_error(format!("malformed wasm: invalid {context} encoding"))
        })?;

        shift += 7;
        if (byte & 0x80) == 0 {
            break;
        }

        if shift >= 35 {
            return Err(malformed_error(format!(
                "malformed wasm: invalid {context} encoding",
            )));
        }

        byte = read_u8(bytes, offset, context)?;
    }

    if shift < 32 && (byte & 0x40) != 0 {
        result |= (!0_i64) << shift;
    }

    if result < i64::from(i32::MIN) || result > i64::from(i32::MAX) {
        return Err(malformed_error(format!(
            "malformed wasm: invalid {context} encoding",
        )));
    }

    Ok(result as i32)
}

fn read_var_i64(bytes: &[u8], offset: &mut usize, context: &str) -> CoreResult<i64> {
    let mut result = 0_i128;
    let mut shift = 0_u32;
    let mut byte = read_u8(bytes, offset, context)?;

    loop {
        result |= i128::from(byte & 0x7f).checked_shl(shift).ok_or_else(|| {
            malformed_error(format!("malformed wasm: invalid {context} encoding"))
        })?;

        shift += 7;
        if (byte & 0x80) == 0 {
            break;
        }

        if shift >= 70 {
            return Err(malformed_error(format!(
                "malformed wasm: invalid {context} encoding",
            )));
        }

        byte = read_u8(bytes, offset, context)?;
    }

    if shift < 64 && (byte & 0x40) != 0 {
        result |= (!0_i128) << shift;
    }

    if result < i128::from(i64::MIN) || result > i128::from(i64::MAX) {
        return Err(malformed_error(format!(
            "malformed wasm: invalid {context} encoding",
        )));
    }

    Ok(result as i64)
}

fn read_name(bytes: &[u8], offset: &mut usize, context: &str) -> CoreResult<String> {
    let len_u32 = read_var_u32(bytes, offset, context)?;
    let len = u32_to_usize(len_u32)?;

    let end = (*offset)
        .checked_add(len)
        .ok_or_else(|| malformed_error("malformed wasm: name length overflow"))?;
    if end > bytes.len() {
        return Err(malformed_error(format!(
            "malformed wasm: {context} extends past section payload"
        )));
    }

    let value = std::str::from_utf8(&bytes[*offset..end]).map_err(|_| {
        malformed_error(format!("malformed wasm: {context} contains invalid UTF-8"))
    })?;
    *offset = end;

    Ok(value.to_owned())
}

fn read_value_type_vec(bytes: &[u8], offset: &mut usize, context: &str) -> CoreResult<Vec<String>> {
    let count_u32 = read_var_u32(bytes, offset, context)?;
    let count = u32_to_usize(count_u32)?;

    // Avoid allocating based on untrusted counts; bounds checks will reject truncated payloads
    // without risking excessive pre-allocation.
    let mut types = Vec::new();
    for _ in 0..count {
        let value_type = read_u8(bytes, offset, context)?;
        types.push(value_type_name(value_type)?.to_owned());
    }

    Ok(types)
}

fn read_block_type(bytes: &[u8], offset: &mut usize) -> CoreResult<()> {
    let block_type = read_u8(bytes, offset, "block type")?;

    match block_type {
        0x40 | 0x7f | 0x7e | 0x7d | 0x7c | 0x7b | 0x70 | 0x6f => Ok(()),
        _ => Err(unsupported_error(
            "unsupported construct: non-MVP block type is not supported",
        )),
    }
}

fn value_type_name(value_type: u8) -> CoreResult<&'static str> {
    match value_type {
        0x7f => Ok("i32"),
        0x7e => Ok("i64"),
        0x7d => Ok("f32"),
        0x7c => Ok("f64"),
        0x7b => Ok("v128"),
        0x70 => Ok("funcref"),
        0x6f => Ok("externref"),
        _ => Err(malformed_error(format!(
            "malformed wasm: invalid value type byte 0x{value_type:02x}",
        ))),
    }
}

fn skip_table_type(bytes: &[u8], offset: &mut usize) -> CoreResult<()> {
    let reference_type = read_u8(bytes, offset, "table reference type")?;
    if reference_type != 0x70 && reference_type != 0x6f {
        return Err(unsupported_error(
            "unsupported construct: table reference type is not supported",
        ));
    }

    skip_limits(bytes, offset)
}

fn skip_memory_type(bytes: &[u8], offset: &mut usize) -> CoreResult<()> {
    skip_limits(bytes, offset)
}

fn skip_limits(bytes: &[u8], offset: &mut usize) -> CoreResult<()> {
    let flags = read_u8(bytes, offset, "limits flags")?;
    match flags {
        0x00 => {
            let _min = read_var_u32(bytes, offset, "limits minimum")?;
            Ok(())
        }
        0x01 => {
            let _min = read_var_u32(bytes, offset, "limits minimum")?;
            let _max = read_var_u32(bytes, offset, "limits maximum")?;
            Ok(())
        }
        _ => Err(unsupported_error(
            "unsupported construct: non-MVP memory/table limits are not supported",
        )),
    }
}

fn skip_global_type(bytes: &[u8], offset: &mut usize) -> CoreResult<()> {
    let value_type = read_u8(bytes, offset, "global value type")?;
    value_type_name(value_type)?;

    let mutability = read_u8(bytes, offset, "global mutability")?;
    if mutability > 1 {
        return Err(malformed_error("malformed wasm: invalid global mutability"));
    }

    Ok(())
}

fn skip_tag_type(bytes: &[u8], offset: &mut usize) -> CoreResult<()> {
    let attribute = read_u8(bytes, offset, "tag attribute")?;
    if attribute != 0 {
        return Err(unsupported_error(
            "unsupported construct: tag attribute is not supported",
        ));
    }

    let _type_index = read_var_u32(bytes, offset, "tag type index")?;
    Ok(())
}

fn ensure_consumed(bytes: &[u8], offset: usize, context: &str) -> CoreResult<()> {
    if offset != bytes.len() {
        return Err(malformed_error(format!(
            "malformed wasm: trailing bytes in {context}"
        )));
    }

    Ok(())
}

fn u32_to_usize(value: u32) -> CoreResult<usize> {
    usize::try_from(value).map_err(|_| {
        malformed_error("malformed wasm: integer value cannot be represented on this platform")
    })
}

fn usize_to_u32(value: usize) -> CoreResult<u32> {
    u32::try_from(value)
        .map_err(|_| internal_error("internal error: integer value exceeds u32 range"))
}

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

use sorcat_core::{
    CoreError, DecodedModuleSummary, Export, ExportKind, FunctionBodySummary, ImportFunction,
    Instruction, SorobanContractSpec, SorobanCustomSections, decode_module_summary,
    decode_soroban_custom_sections,
};
use sorcat_soroban_knowledge::resolve_imports as resolve_soroban_knowledge_imports;
use wasmparser::{Parser, Payload, TypeRef, ValType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RustBackendError {
    Core(CoreError),
    InvalidInput {
        field: &'static str,
        message: String,
    },
}

impl Display for RustBackendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(source) => write!(f, "core decode failed: {}", source.message),
            Self::InvalidInput { field, message } => {
                write!(f, "invalid input for `{field}`: {message}")
            }
        }
    }
}

impl std::error::Error for RustBackendError {}

impl From<CoreError> for RustBackendError {
    fn from(value: CoreError) -> Self {
        Self::Core(value)
    }
}

pub fn reconstruct_module_from_wasm(wasm: &[u8]) -> Result<String, RustBackendError> {
    let summary = decode_module_summary(wasm)?;
    let soroban_sections = decode_soroban_custom_sections(wasm)?;
    let wasm_context = parse_wasm_module_context(wasm)?;
    reconstruct_module_with_wasm_context(&summary, &wasm_context, &soroban_sections)
}

pub fn reconstruct_fixture_module_from_wasm(
    wasm: &[u8],
    contract_id: &str,
    sequence: u64,
) -> Result<String, RustBackendError> {
    let summary = decode_module_summary(wasm)?;
    reconstruct_fixture_module(&summary, contract_id, sequence)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WasmImportFunction {
    module: String,
    name: String,
    params: Vec<String>,
    results: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WasmFunctionSignature {
    params: Vec<String>,
    results: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WasmModuleContext {
    imports: Vec<WasmImportFunction>,
    signatures: Vec<WasmFunctionSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallTarget {
    ident: String,
    signature: WasmFunctionSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportRenderPlan {
    raw_ident: String,
    wrapper_ident: String,
    signature: WasmFunctionSignature,
    canonical_id: Option<String>,
    semantic_tags: Vec<String>,
    resolution_reason: String,
}

fn parse_wasm_module_context(wasm: &[u8]) -> Result<WasmModuleContext, RustBackendError> {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawFunctionImport {
        module: String,
        name: String,
        type_index: u32,
    }

    let mut types: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    let mut raw_imports = Vec::new();
    let mut defined_type_indices = Vec::new();

    for payload in Parser::new(0).parse_all(wasm) {
        let payload = payload.map_err(|error| RustBackendError::InvalidInput {
            field: "wasm",
            message: format!("wasmparser failed: {error}"),
        })?;

        match payload {
            Payload::TypeSection(reader) => {
                types = reader
                    .into_iter_err_on_gc_types()
                    .map(|func| {
                        let func = func.map_err(|error| RustBackendError::InvalidInput {
                            field: "wasm.type_section",
                            message: format!("failed to parse type section: {error}"),
                        })?;
                        let params = func.params().iter().map(render_valtype).collect();
                        let results = func.results().iter().map(render_valtype).collect();
                        Ok((params, results))
                    })
                    .collect::<Result<Vec<_>, RustBackendError>>()?;
            }
            Payload::ImportSection(reader) => {
                for import in reader {
                    let import = import.map_err(|error| RustBackendError::InvalidInput {
                        field: "wasm.import_section",
                        message: format!("failed to parse import section: {error}"),
                    })?;
                    let TypeRef::Func(type_index) = import.ty else {
                        continue;
                    };
                    raw_imports.push(RawFunctionImport {
                        module: import.module.to_string(),
                        name: import.name.to_string(),
                        type_index,
                    });
                }
            }
            Payload::FunctionSection(reader) => {
                for entry in reader {
                    let type_index = entry.map_err(|error| RustBackendError::InvalidInput {
                        field: "wasm.function_section",
                        message: format!("failed to parse function section: {error}"),
                    })?;
                    defined_type_indices.push(type_index);
                }
            }
            _ => {}
        }
    }

    let mut imports = Vec::with_capacity(raw_imports.len());
    let mut signatures = Vec::with_capacity(raw_imports.len() + defined_type_indices.len());

    for import in raw_imports {
        let signature = resolve_type_signature(&types, import.type_index, "wasm.import_section.type_index")?;
        imports.push(WasmImportFunction {
            module: import.module,
            name: import.name,
            params: signature.params.clone(),
            results: signature.results.clone(),
        });
        signatures.push(signature);
    }

    for type_index in defined_type_indices {
        signatures.push(resolve_type_signature(
            &types,
            type_index,
            "wasm.function_section.type_index",
        )?);
    }

    Ok(WasmModuleContext { imports, signatures })
}

fn resolve_type_signature(
    types: &[(Vec<String>, Vec<String>)],
    type_index: u32,
    field: &'static str,
) -> Result<WasmFunctionSignature, RustBackendError> {
    let idx = usize::try_from(type_index).map_err(|_| RustBackendError::InvalidInput {
        field,
        message: format!("unsupported type index {type_index}"),
    })?;
    let (params, results) = types.get(idx).ok_or_else(|| RustBackendError::InvalidInput {
        field,
        message: format!("missing type for index {type_index}"),
    })?;

    Ok(WasmFunctionSignature {
        params: params.clone(),
        results: results.clone(),
    })
}

fn render_valtype(value: &ValType) -> String {
    value.to_string()
}

pub fn reconstruct_module(summary: &DecodedModuleSummary) -> Result<String, RustBackendError> {
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
    exports.sort_by(|left, right| left.name.cmp(&right.name));

    let mut bodies = summary.function_bodies.clone();
    bodies.sort_by(|left, right| left.symbol.cmp(&right.symbol));

    let mut used_import_idents = BTreeMap::<String, usize>::new();
    let mut rendered_imports = Vec::with_capacity(imports.len());
    for import in imports {
        let base_ident = sanitize_ident(&format!("{}_{}", import.module, import.name));
        let ident = make_unique_ident(base_ident, &mut used_import_idents);
        rendered_imports.push(render_import_line(import, ident));
    }

    let mut used_function_idents = BTreeMap::<String, usize>::new();
    let mut rendered_functions = Vec::new();
    for body in bodies {
        let base_ident = sanitize_ident(&body.symbol);
        let function_name = make_unique_ident(base_ident, &mut used_function_idents);
        rendered_functions.extend(render_function_block(body, function_name));
    }

    let mut lines = Vec::new();
    lines.push("// sorcat deterministic pseudo-rust summary v0".to_string());
    lines.push("pub mod decompiled {".to_string());
    lines.push(format!(
        "    // defined function bodies: {}",
        summary.defined_function_bodies
    ));

    lines.push(format!("    // imports: {}", summary.import_functions.len()));
    if rendered_imports.is_empty() {
        lines.push("    // (none)".to_string());
    } else {
        lines.push("    extern \"C\" {".to_string());
        lines.extend(rendered_imports);
        lines.push("    }".to_string());
    }

    lines.push(format!("    // exports: {}", exports.len()));
    lines.extend(exports.into_iter().map(render_export_comment_line));

    lines.push(format!(
        "    // reconstructed functions: {}",
        summary.function_bodies.len()
    ));
    lines.extend(rendered_functions);
    lines.push("}".to_string());

    Ok(lines.join("\n"))
}

fn reconstruct_module_with_wasm_context(
    summary: &DecodedModuleSummary,
    wasm_context: &WasmModuleContext,
    soroban_sections: &SorobanCustomSections,
) -> Result<String, RustBackendError> {
    validate_summary(summary)?;

    let mut exports = summary.exports.clone();
    exports.sort_by(|left, right| left.name.cmp(&right.name));
    let exported_function_indices = summary
        .exports
        .iter()
        .filter(|export| export.kind == ExportKind::Function)
        .map(|export| export.index)
        .collect::<BTreeSet<_>>();

    let mut bodies = summary.function_bodies.clone();
    bodies.sort_by(|left, right| left.symbol.cmp(&right.symbol));

    let import_resolutions = resolve_soroban_knowledge_imports(
        wasm_context
            .imports
            .iter()
            .map(|import| (import.module.clone(), import.name.clone())),
    );
    let mut resolution_by_key = BTreeMap::new();
    for resolution in import_resolutions {
        resolution_by_key
            .entry((resolution.module.clone(), resolution.name.clone()))
            .or_insert(resolution);
    }

    let mut used_import_idents = BTreeMap::<String, usize>::new();
    let mut used_wrapper_idents = BTreeMap::<String, usize>::new();
    let mut import_plans = Vec::<ImportRenderPlan>::with_capacity(wasm_context.imports.len());
    let mut rendered_imports = Vec::with_capacity(wasm_context.imports.len());
    for (import_index, import) in wasm_context.imports.iter().enumerate() {
        let index_u32 = u32::try_from(import_index).map_err(|_| RustBackendError::InvalidInput {
            field: "wasm.imports",
            message: format!("unsupported import index {import_index}"),
        })?;

        let signature = signature_for_function_index(wasm_context, index_u32)?;
        let base_ident = sanitize_ident(&format!("{}_{}", import.module, import.name));
        let raw_ident = make_unique_ident(base_ident, &mut used_import_idents);
        rendered_imports.push(render_wasm_import_line(import, &raw_ident));

        let resolution = resolution_by_key
            .get(&(import.module.clone(), import.name.clone()))
            .cloned();
        let wrapper_base = resolution
            .as_ref()
            .and_then(|entry| entry.canonical_id.as_ref())
            .map(|id| sanitize_ident(&format!("host_{}", id.replace('.', "_"))))
            .unwrap_or_else(|| sanitize_ident(&format!("host_{}_{}", import.module, import.name)));
        let wrapper_ident = make_unique_ident(wrapper_base, &mut used_wrapper_idents);

        import_plans.push(ImportRenderPlan {
            raw_ident,
            wrapper_ident,
            signature,
            canonical_id: resolution.as_ref().and_then(|entry| entry.canonical_id.clone()),
            semantic_tags: resolution
                .as_ref()
                .map(|entry| entry.semantic_tags.clone())
                .unwrap_or_default(),
            resolution_reason: resolution
                .as_ref()
                .map(|entry| entry.reason.clone())
                .unwrap_or_else(|| "no soroban resolution metadata".to_string()),
        });
    }

    let mut used_function_idents = BTreeMap::<String, usize>::new();
    let mut planned_functions = Vec::with_capacity(bodies.len());
    for body in bodies {
        let base_ident = sanitize_ident(&body.symbol);
        let function_name = make_unique_ident(base_ident, &mut used_function_idents);
        let signature = signature_for_function_index(wasm_context, body.function_index)?;
        let is_public = exported_function_indices.contains(&body.function_index);
        planned_functions.push((body, function_name, signature, is_public));
    }

    let mut call_targets = BTreeMap::<u32, CallTarget>::new();
    for (import_index, plan) in import_plans.iter().enumerate() {
        let index_u32 = u32::try_from(import_index).map_err(|_| RustBackendError::InvalidInput {
            field: "wasm.imports",
            message: format!("unsupported import index {import_index}"),
        })?;
        call_targets.insert(
            index_u32,
            CallTarget {
                ident: plan.wrapper_ident.clone(),
                signature: plan.signature.clone(),
            },
        );
    }
    for (body, function_name, signature, _is_public) in &planned_functions {
        call_targets.insert(
            body.function_index,
            CallTarget {
                ident: function_name.clone(),
                signature: signature.clone(),
            },
        );
    }

    let mut rendered_functions = Vec::new();
    let spec_by_name = contract_spec_functions_by_name(soroban_sections.contract_spec.as_ref());
    for (body, function_name, signature, is_public) in planned_functions {
        let spec_function = spec_by_name.get(body.symbol.as_str());
        rendered_functions.extend(render_reconstructed_function_block(
            body,
            function_name,
            &signature,
            &call_targets,
            spec_function.copied(),
            is_public,
        ));
    }

    let mut lines = Vec::new();
    lines.push("// sorcat deterministic rust reconstruction v2".to_string());
    lines.push("pub mod decompiled {".to_string());
    lines.push(format!(
        "    // defined function bodies: {}",
        summary.defined_function_bodies
    ));

    lines.extend(render_soroban_contract_context(soroban_sections));
    lines.extend(render_soroban_spec_artifacts(soroban_sections.contract_spec.as_ref()));

    lines.push(format!("    // imports: {}", wasm_context.imports.len()));
    if rendered_imports.is_empty() {
        lines.push("    // (none)".to_string());
    } else {
        lines.push("    extern \"C\" {".to_string());
        lines.extend(rendered_imports);
        lines.push("    }".to_string());
        lines.push(String::new());
        lines.push("    // host wrappers".to_string());
        for plan in &import_plans {
            lines.extend(render_host_wrapper_block(plan));
        }
    }

    lines.push(format!("    // exports: {}", exports.len()));
    lines.extend(exports.into_iter().map(render_export_comment_line));

    lines.push(format!(
        "    // reconstructed functions: {}",
        summary.function_bodies.len()
    ));
    lines.extend(rendered_functions);
    lines.push("}".to_string());

    Ok(lines.join("\n"))
}

fn signature_for_function_index(
    wasm_context: &WasmModuleContext,
    function_index: u32,
) -> Result<WasmFunctionSignature, RustBackendError> {
    let index = usize::try_from(function_index).map_err(|_| RustBackendError::InvalidInput {
        field: "wasm.function_index",
        message: format!("unsupported function index {function_index}"),
    })?;
    wasm_context
        .signatures
        .get(index)
        .cloned()
        .ok_or_else(|| RustBackendError::InvalidInput {
            field: "wasm.function_signatures",
            message: format!("missing signature for function index {function_index}"),
        })
}

fn render_wasm_import_line(import: &WasmImportFunction, ident: &str) -> String {
    let params = import
        .params
        .iter()
        .enumerate()
        .map(|(idx, ty)| format!("arg{idx}: {}", wasm_type_to_rust(ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = render_return_type(&import.results);

    if params.is_empty() {
        format!("        fn {ident}(){return_type};")
    } else {
        format!("        fn {ident}({params}){return_type};")
    }
}

fn render_host_wrapper_block(plan: &ImportRenderPlan) -> Vec<String> {
    let params = plan
        .signature
        .params
        .iter()
        .enumerate()
        .map(|(idx, ty)| format!("arg{idx}: {}", wasm_type_to_rust(ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let args = (0..plan.signature.params.len())
        .map(|idx| format!("arg{idx}"))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = render_return_type(&plan.signature.results);

    let mut lines = Vec::new();
    if let Some(canonical_id) = &plan.canonical_id {
        lines.push(format!(
            "    // soroban_builtin={} tags=[{}]",
            canonical_id,
            plan.semantic_tags.join(", ")
        ));
    } else {
        lines.push(format!(
            "    // unresolved_import tags=[{}]",
            plan.semantic_tags.join(", ")
        ));
    }
    lines.push(format!("    // resolution_reason={}", plan.resolution_reason));

    if params.is_empty() {
        lines.push(format!(
            "    pub fn {}(){return_type} {{",
            plan.wrapper_ident
        ));
        if plan.signature.results.is_empty() {
            lines.push(format!("        unsafe {{ {}(); }}", plan.raw_ident));
        } else {
            lines.push(format!("        unsafe {{ {}() }}", plan.raw_ident));
        }
    } else {
        lines.push(format!(
            "    pub fn {}({params}){return_type} {{",
            plan.wrapper_ident
        ));
        if plan.signature.results.is_empty() {
            lines.push(format!("        unsafe {{ {}({args}); }}", plan.raw_ident));
        } else {
            lines.push(format!("        unsafe {{ {}({args}) }}", plan.raw_ident));
        }
    }
    lines.push("    }".to_string());
    lines.push(String::new());
    lines
}

fn contract_spec_functions_by_name<'a>(
    spec: Option<&'a SorobanContractSpec>,
) -> BTreeMap<String, &'a sorcat_core::SorobanSpecFunction> {
    let mut by_name = BTreeMap::new();
    if let Some(spec) = spec {
        for function in &spec.functions {
            by_name.insert(function.name.clone(), function);
        }
    }
    by_name
}

fn render_soroban_contract_context(sections: &SorobanCustomSections) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(meta) = &sections.contract_meta {
        if let Some(name) = &meta.contract_name {
            lines.push(format!("    // soroban.contract_name={name}"));
        }
        if let Some(version) = &meta.version {
            lines.push(format!("    // soroban.contract_version={version}"));
        }
        for entry in &meta.entries {
            lines.push(format!(
                "    // soroban.meta.{}={}",
                sanitize_ident(&entry.key),
                entry.value
            ));
        }
    }
    if let Some(env_meta) = &sections.contract_env_meta {
        lines.push(format!(
            "    // soroban.protocol_min={} soroban.protocol_max={}",
            env_meta.protocol, env_meta.protocol
        ));
        if let Some(interface_version) = env_meta.interface_version {
            lines.push(format!(
                "    // soroban.interface_version={interface_version}"
            ));
        }
        if let Some(sdk_version) = &env_meta.sdk_version {
            lines.push(format!("    // soroban.sdk_version={sdk_version}"));
        }
    }
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn render_soroban_spec_artifacts(spec: Option<&SorobanContractSpec>) -> Vec<String> {
    let Some(spec) = spec else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    lines.push("    // soroban typed artifacts".to_string());
    for ty in &spec.types {
        match ty.kind {
            sorcat_core::SorobanSpecTypeKind::Struct => {
                lines.push("    #[derive(Clone, Debug, PartialEq, Eq)]".to_string());
                lines.push(format!("    pub struct {} {{", sanitize_type_name(&ty.name)));
                if ty.fields.is_empty() {
                    lines.push("        pub _unused: i32,".to_string());
                } else {
                    for field in &ty.fields {
                        lines.push(format!(
                            "        pub {}: {},",
                            sanitize_ident(&field.name),
                            soroban_type_to_rust(&field.ty)
                        ));
                    }
                }
                lines.push("    }".to_string());
                lines.push(String::new());
            }
            sorcat_core::SorobanSpecTypeKind::Enum => {
                lines.push("    #[derive(Clone, Debug, PartialEq, Eq)]".to_string());
                lines.push(format!("    pub enum {} {{", sanitize_type_name(&ty.name)));
                if ty.variants.is_empty() {
                    lines.push("        Unknown,".to_string());
                } else {
                    for variant in &ty.variants {
                        lines.push(format!("        {},", sanitize_type_name(variant)));
                    }
                }
                lines.push("    }".to_string());
                lines.push(String::new());
            }
            sorcat_core::SorobanSpecTypeKind::Alias => {
                let target = ty
                    .alias_target
                    .as_deref()
                    .map(soroban_type_to_rust)
                    .unwrap_or("i64");
                lines.push(format!(
                    "    pub type {} = {target};",
                    sanitize_type_name(&ty.name)
                ));
                lines.push(String::new());
            }
        }
    }

    if !spec.errors.is_empty() {
        lines.push("    #[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_string());
        lines.push("    #[repr(i32)]".to_string());
        lines.push("    pub enum ContractError {".to_string());
        for error in &spec.errors {
            lines.push(format!(
                "        {} = {},",
                sanitize_type_name(&error.name),
                error.code
            ));
        }
        lines.push("    }".to_string());
        lines.push(String::new());
    }

    lines
}

fn render_function_params(
    wasm_signature: &WasmFunctionSignature,
    spec_function: Option<&sorcat_core::SorobanSpecFunction>,
) -> String {
    if let Some(spec) = spec_function {
        if spec.inputs.len() == wasm_signature.params.len() {
            return spec
                .inputs
                .iter()
                .map(|input| {
                    format!(
                        "{}: {}",
                        sanitize_ident(&input.name),
                        soroban_type_to_rust(&input.ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
        }
    }

    wasm_signature
        .params
        .iter()
        .enumerate()
        .map(|(idx, ty)| format!("arg{idx}: {}", wasm_type_to_rust(ty)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_function_return_type(
    wasm_signature: &WasmFunctionSignature,
    spec_function: Option<&sorcat_core::SorobanSpecFunction>,
) -> String {
    if let Some(spec) = spec_function {
        if let Some(output) = &spec.output {
            return format!(" -> {}", soroban_type_to_rust(output));
        }
    }
    render_return_type(&wasm_signature.results)
}

fn render_reconstructed_function_block(
    body: FunctionBodySummary,
    function_name: String,
    signature: &WasmFunctionSignature,
    call_targets: &BTreeMap<u32, CallTarget>,
    spec_function: Option<&sorcat_core::SorobanSpecFunction>,
    is_public: bool,
) -> Vec<String> {
    let params = render_function_params(signature, spec_function);
    let return_type = render_function_return_type(signature, spec_function);

    let mut lines = Vec::new();
    let visibility = if is_public { "pub " } else { "" };
    if params.is_empty() {
        lines.push(format!("    {visibility}fn {function_name}(){return_type} {{"));
    } else {
        lines.push(format!(
            "    {visibility}fn {function_name}({params}){return_type} {{"
        ));
    }

    for line in render_reconstructed_function_body_lines(&body, signature, call_targets) {
        lines.push(format!("        {line}"));
    }

    lines.push("    }".to_string());
    lines.push(String::new());
    lines
}

fn render_reconstructed_function_body_lines(
    body: &FunctionBodySummary,
    signature: &WasmFunctionSignature,
    call_targets: &BTreeMap<u32, CallTarget>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut stack = Vec::<String>::new();
    let mut local_bindings = BTreeMap::<u32, String>::new();
    let mut global_bindings = BTreeMap::<u32, String>::new();
    let mut control_stack = Vec::<ControlFrame>::new();
    let mut control_indent = 0usize;
    let mut next_control_label = 0usize;
    let mut has_explicit_return = false;
    let param_count_u32 = u32::try_from(signature.params.len()).unwrap_or(u32::MAX);

    for instruction in &body.instructions {
        match instruction {
            Instruction::LocalGet { local_index } => {
                if *local_index < param_count_u32 {
                    stack.push(format!("arg{local_index}"));
                } else if let Some(local_name) = local_bindings.get(local_index) {
                    stack.push(local_name.clone());
                } else {
                    stack.push(default_literal_for_wasm_type("i32"));
                }
            }
            Instruction::LocalSet { local_index } => {
                let value = pop_or_default(&mut stack, "i32");
                let local_name = if *local_index < param_count_u32 {
                    format!("arg{local_index}")
                } else {
                    format!("local{local_index}")
                };
                lines.push(format!("let {local_name} = {value};"));
                local_bindings.insert(*local_index, local_name);
            }
            Instruction::LocalTee { local_index } => {
                let value = pop_or_default(&mut stack, "i32");
                let local_name = if *local_index < param_count_u32 {
                    format!("arg{local_index}")
                } else {
                    format!("local{local_index}")
                };
                lines.push(format!("let {local_name} = {value};"));
                local_bindings.insert(*local_index, local_name.clone());
                stack.push(local_name);
            }
            Instruction::GlobalGet { global_index } => {
                if let Some(global_name) = global_bindings.get(global_index) {
                    stack.push(global_name.clone());
                } else {
                    stack.push(default_literal_for_wasm_type("i64"));
                }
            }
            Instruction::GlobalSet { global_index } => {
                let value = pop_or_default(&mut stack, "i64");
                let global_name = format!("global{global_index}");
                lines.push(format!("let {global_name} = {value};"));
                global_bindings.insert(*global_index, global_name);
            }
            Instruction::I32Const { value } => stack.push(value.to_string()),
            Instruction::I64Const { value } => stack.push(format!("{value}_i64")),
            Instruction::I32Eq => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("(({left} == {right}) as i32)"));
            }
            Instruction::I32Ne => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("(({left} != {right}) as i32)"));
            }
            Instruction::I32LtS => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("(({left} < {right}) as i32)"));
            }
            Instruction::I32LtU => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("((({left} as u32) < ({right} as u32)) as i32)"));
            }
            Instruction::I32GtS => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("(({left} > {right}) as i32)"));
            }
            Instruction::I32GtU => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("((({left} as u32) > ({right} as u32)) as i32)"));
            }
            Instruction::I32LeS => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("(({left} <= {right}) as i32)"));
            }
            Instruction::I32LeU => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("((({left} as u32) <= ({right} as u32)) as i32)"));
            }
            Instruction::I32GeS => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("(({left} >= {right}) as i32)"));
            }
            Instruction::I32GeU => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("((({left} as u32) >= ({right} as u32)) as i32)"));
            }
            Instruction::I32Eqz => {
                let value = pop_or_default(&mut stack, "i32");
                stack.push(format!("(({value} == 0) as i32)"));
            }
            Instruction::I64Eqz => {
                let value = pop_or_default(&mut stack, "i64");
                stack.push(format!("(({value} == 0) as i32)"));
            }
            Instruction::I64Eq => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("(({left} == {right}) as i32)"));
            }
            Instruction::I64Ne => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("(({left} != {right}) as i32)"));
            }
            Instruction::I64LtS => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("(({left} < {right}) as i32)"));
            }
            Instruction::I64LtU => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("((({left} as u64) < ({right} as u64)) as i32)"));
            }
            Instruction::I64GtS => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("(({left} > {right}) as i32)"));
            }
            Instruction::I64GtU => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("((({left} as u64) > ({right} as u64)) as i32)"));
            }
            Instruction::I64LeS => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("(({left} <= {right}) as i32)"));
            }
            Instruction::I64LeU => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("((({left} as u64) <= ({right} as u64)) as i32)"));
            }
            Instruction::I64GeS => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("(({left} >= {right}) as i32)"));
            }
            Instruction::I64GeU => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("((({left} as u64) >= ({right} as u64)) as i32)"));
            }
            Instruction::I32Add => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} + {right})"));
            }
            Instruction::I32Sub => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} - {right})"));
            }
            Instruction::I32Mul => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} * {right})"));
            }
            Instruction::I32DivS => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} / {right})"));
            }
            Instruction::I32DivU => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("((({left} as u32) / ({right} as u32)) as i32)"));
            }
            Instruction::I32RemS => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} % {right})"));
            }
            Instruction::I32RemU => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("((({left} as u32) % ({right} as u32)) as i32)"));
            }
            Instruction::I32And => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} & {right})"));
            }
            Instruction::I32Or => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} | {right})"));
            }
            Instruction::I32Xor => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} ^ {right})"));
            }
            Instruction::I32Shl => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} << ({right} as u32))"));
            }
            Instruction::I32ShrS => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("({left} >> ({right} as u32))"));
            }
            Instruction::I32ShrU => {
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("((({left} as u32) >> ({right} as u32)) as i32)"));
            }
            Instruction::I64Add => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} + {right})"));
            }
            Instruction::I64Sub => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} - {right})"));
            }
            Instruction::I64Mul => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} * {right})"));
            }
            Instruction::I64DivS => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} / {right})"));
            }
            Instruction::I64DivU => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("((({left} as u64) / ({right} as u64)) as i64)"));
            }
            Instruction::I64RemS => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} % {right})"));
            }
            Instruction::I64RemU => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("((({left} as u64) % ({right} as u64)) as i64)"));
            }
            Instruction::I64And => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} & {right})"));
            }
            Instruction::I64Or => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} | {right})"));
            }
            Instruction::I64Xor => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} ^ {right})"));
            }
            Instruction::I64Shl => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} << ({right} as u32))"));
            }
            Instruction::I64ShrS => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("({left} >> ({right} as u32))"));
            }
            Instruction::I64ShrU => {
                let right = pop_or_default(&mut stack, "i64");
                let left = pop_or_default(&mut stack, "i64");
                stack.push(format!("((({left} as u64) >> ({right} as u32)) as i64)"));
            }
            Instruction::Call { function_index } => {
                if let Some(target) = call_targets.get(function_index) {
                    let mut args = Vec::with_capacity(target.signature.params.len());
                    for ty in target.signature.params.iter().rev() {
                        args.push(pop_or_default(&mut stack, ty));
                    }
                    args.reverse();

                    let call_expr = if args.is_empty() {
                        format!("{}()", target.ident)
                    } else {
                        format!("{}({})", target.ident, args.join(", "))
                    };

                    if target.signature.results.is_empty() {
                        lines.push(format!("{call_expr};"));
                    } else {
                        stack.push(call_expr);
                    }
                } else {
                    lines.push(format!(
                        "// unsupported instruction: call target index {function_index} was not resolved"
                    ));
                }
            }
            Instruction::Select => {
                let condition = pop_or_default(&mut stack, "i32");
                let right = pop_or_default(&mut stack, "i32");
                let left = pop_or_default(&mut stack, "i32");
                stack.push(format!("if ({condition}) != 0 {{ {left} }} else {{ {right} }}"));
            }
            Instruction::BrTable {
                targets,
                default_target,
            } => {
                let selector = pop_or_default(&mut stack, "i32");
                push_indented_line(&mut lines, control_indent, format!("match {selector} {{"));
                for (index, target_depth) in targets.iter().enumerate() {
                    push_indented_line(&mut lines, control_indent + 1, format!("{index} => {{"));
                    match resolve_branch_action(*target_depth, &control_stack) {
                        BranchAction::Continue { label } => {
                            push_indented_line(
                                &mut lines,
                                control_indent + 2,
                                format!("continue '{label};"),
                            );
                        }
                        BranchAction::Break { label } => {
                            push_indented_line(
                                &mut lines,
                                control_indent + 2,
                                format!("break '{label};"),
                            );
                        }
                        BranchAction::Unsupported { detail } => {
                            push_indented_line(
                                &mut lines,
                                control_indent + 2,
                                format!(
                                    "// unsupported br_table target depth={target_depth} ({detail})"
                                ),
                            );
                        }
                    }
                    push_indented_line(&mut lines, control_indent + 1, "},");
                }
                push_indented_line(&mut lines, control_indent + 1, "_ => {");
                match resolve_branch_action(*default_target, &control_stack) {
                    BranchAction::Continue { label } => {
                        push_indented_line(
                            &mut lines,
                            control_indent + 2,
                            format!("continue '{label};"),
                        );
                    }
                    BranchAction::Break { label } => {
                        push_indented_line(
                            &mut lines,
                            control_indent + 2,
                            format!("break '{label};"),
                        );
                    }
                    BranchAction::Unsupported { detail } => {
                        push_indented_line(
                            &mut lines,
                            control_indent + 2,
                            format!(
                                "// unsupported br_table default target depth={default_target} ({detail})"
                            ),
                        );
                    }
                }
                push_indented_line(&mut lines, control_indent + 1, "},");
                push_indented_line(&mut lines, control_indent, "}");
            }
            Instruction::Drop => {
                let value = pop_or_default(&mut stack, "i32");
                push_indented_line(&mut lines, control_indent, format!("let _ = {value};"));
            }
            Instruction::Return => {
                has_explicit_return = true;
                match signature.results.len() {
                    0 => push_indented_line(&mut lines, control_indent, "return;"),
                    1 => {
                        let value = pop_or_default(&mut stack, &signature.results[0]);
                        push_indented_line(&mut lines, control_indent, format!("return {value};"));
                    }
                    _ => {
                        let mut values = Vec::with_capacity(signature.results.len());
                        for ty in signature.results.iter().rev() {
                            values.push(pop_or_default(&mut stack, ty));
                        }
                        values.reverse();
                        push_indented_line(
                            &mut lines,
                            control_indent,
                            format!("return ({});", values.join(", ")),
                        );
                    }
                }
            }
            Instruction::Block => {
                let label = format!("cf_{next_control_label}");
                next_control_label = next_control_label.saturating_add(1);
                push_indented_line(&mut lines, control_indent, format!("'{label}: {{"));
                control_stack.push(ControlFrame {
                    kind: ControlFrameKind::Block,
                    label,
                });
                control_indent = control_indent.saturating_add(1);
            }
            Instruction::Loop => {
                let label = format!("cf_{next_control_label}");
                next_control_label = next_control_label.saturating_add(1);
                push_indented_line(&mut lines, control_indent, format!("'{label}: loop {{"));
                control_stack.push(ControlFrame {
                    kind: ControlFrameKind::Loop,
                    label,
                });
                control_indent = control_indent.saturating_add(1);
            }
            Instruction::If => {
                let condition = pop_or_default(&mut stack, "i32");
                push_indented_line(
                    &mut lines,
                    control_indent,
                    format!("if ({condition}) != 0 {{"),
                );
                control_stack.push(ControlFrame {
                    kind: ControlFrameKind::If { seen_else: false },
                    label: format!("if_{next_control_label}"),
                });
                next_control_label = next_control_label.saturating_add(1);
                control_indent = control_indent.saturating_add(1);
            }
            Instruction::Else => {
                match control_stack.last_mut() {
                    Some(ControlFrame {
                        kind: ControlFrameKind::If { seen_else },
                        ..
                    }) if !*seen_else => {
                        *seen_else = true;
                        control_indent = control_indent.saturating_sub(1);
                        push_indented_line(&mut lines, control_indent, "} else {");
                        control_indent = control_indent.saturating_add(1);
                    }
                    _ => {
                        push_indented_line(
                            &mut lines,
                            control_indent,
                            "// unsupported instruction: else (structural control mismatch)",
                        );
                    }
                }
            }
            Instruction::Br { depth } => {
                match resolve_branch_action(*depth, &control_stack) {
                    BranchAction::Continue { label } => {
                        push_indented_line(
                            &mut lines,
                            control_indent,
                            format!("continue '{label};"),
                        );
                    }
                    BranchAction::Break { label } => {
                        push_indented_line(&mut lines, control_indent, format!("break '{label};"));
                    }
                    BranchAction::Unsupported { detail } => {
                        push_indented_line(
                            &mut lines,
                            control_indent,
                            format!("// unsupported instruction: br depth={depth} ({detail})"),
                        );
                    }
                }
            }
            Instruction::BrIf { depth } => {
                let condition = pop_or_default(&mut stack, "i32");
                match resolve_branch_action(*depth, &control_stack) {
                    BranchAction::Continue { label } => {
                        push_indented_line(
                            &mut lines,
                            control_indent,
                            format!("if ({condition}) != 0 {{"),
                        );
                        push_indented_line(
                            &mut lines,
                            control_indent + 1,
                            format!("continue '{label};"),
                        );
                        push_indented_line(&mut lines, control_indent, "}");
                    }
                    BranchAction::Break { label } => {
                        push_indented_line(
                            &mut lines,
                            control_indent,
                            format!("if ({condition}) != 0 {{"),
                        );
                        push_indented_line(
                            &mut lines,
                            control_indent + 1,
                            format!("break '{label};"),
                        );
                        push_indented_line(&mut lines, control_indent, "}");
                    }
                    BranchAction::Unsupported { detail } => {
                        push_indented_line(
                            &mut lines,
                            control_indent,
                            format!(
                                "// unsupported instruction: br_if depth={depth} ({detail}), condition={condition}"
                            ),
                        );
                    }
                }
            }
            Instruction::CallIndirect {
                type_index,
                table_index,
            } => {
                push_indented_line(
                    &mut lines,
                    control_indent,
                    format!(
                    "// unsupported instruction: call_indirect type_index={type_index} table_index={table_index}"
                    ),
                );
            }
            Instruction::End => {
                if control_stack.pop().is_some() {
                    control_indent = control_indent.saturating_sub(1);
                    push_indented_line(&mut lines, control_indent, "}");
                }
            }
        }
    }

    while control_stack.pop().is_some() {
        control_indent = control_indent.saturating_sub(1);
        push_indented_line(&mut lines, control_indent, "}");
    }

    if !has_explicit_return {
        match signature.results.len() {
            0 => {
                for value in stack {
                    push_indented_line(&mut lines, control_indent, format!("let _ = {value};"));
                }
            }
            1 => {
                let result = pop_or_default(&mut stack, &signature.results[0]);
                for value in stack {
                    push_indented_line(&mut lines, control_indent, format!("let _ = {value};"));
                }
                push_indented_line(&mut lines, control_indent, result);
            }
            _ => {
                let mut values = Vec::with_capacity(signature.results.len());
                for ty in signature.results.iter().rev() {
                    values.push(pop_or_default(&mut stack, ty));
                }
                values.reverse();
                for value in stack {
                    push_indented_line(&mut lines, control_indent, format!("let _ = {value};"));
                }
                push_indented_line(&mut lines, control_indent, format!("({})", values.join(", ")));
            }
        }
    }

    lines
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlFrame {
    kind: ControlFrameKind,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ControlFrameKind {
    Block,
    Loop,
    If { seen_else: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BranchAction {
    Continue { label: String },
    Break { label: String },
    Unsupported { detail: String },
}

fn resolve_branch_action(depth: u32, control_stack: &[ControlFrame]) -> BranchAction {
    let Ok(depth_usize) = usize::try_from(depth) else {
        return BranchAction::Unsupported {
            detail: "branch depth is too large for platform usize".to_string(),
        };
    };

    let Some(offset) = depth_usize.checked_add(1) else {
        return BranchAction::Unsupported {
            detail: "branch depth overflow".to_string(),
        };
    };
    let Some(target_index) = control_stack.len().checked_sub(offset) else {
        return BranchAction::Unsupported {
            detail: "branch depth exceeds active control nesting".to_string(),
        };
    };

    let target = &control_stack[target_index];
    match &target.kind {
        ControlFrameKind::Loop => BranchAction::Continue {
            label: target.label.clone(),
        },
        ControlFrameKind::Block => BranchAction::Break {
            label: target.label.clone(),
        },
        ControlFrameKind::If { .. } => BranchAction::Unsupported {
            detail: "if-target branches are not reconstructed yet".to_string(),
        },
    }
}

fn push_indented_line(lines: &mut Vec<String>, indent: usize, line: impl AsRef<str>) {
    lines.push(format!("{}{}", "    ".repeat(indent), line.as_ref()));
}

fn pop_or_default(stack: &mut Vec<String>, wasm_type: &str) -> String {
    stack
        .pop()
        .unwrap_or_else(|| default_literal_for_wasm_type(wasm_type))
}

fn default_literal_for_wasm_type(wasm_type: &str) -> String {
    match wasm_type {
        "i64" => "0_i64".to_string(),
        "f32" => "0.0_f32".to_string(),
        "f64" => "0.0_f64".to_string(),
        _ => "0".to_string(),
    }
}

pub fn reconstruct_fixture_module(
    summary: &DecodedModuleSummary,
    contract_id: &str,
    sequence: u64,
) -> Result<String, RustBackendError> {
    validate_summary(summary)?;

    if contract_id.trim().is_empty() {
        return Err(RustBackendError::InvalidInput {
            field: "contract_id",
            message: "must not be empty".to_string(),
        });
    }

    let operator = inferred_fixture_operator(summary);
    let mut lines = Vec::new();
    lines.push("#![allow(dead_code)]".to_string());
    lines.push(String::new());
    lines.push("pub fn fixture_contract_id() -> &'static str {".to_string());
    lines.push(format!("    \"{contract_id}\""));
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("pub fn fixture_entrypoint(input: i64) -> i64 {".to_string());
    lines.push(format!("    input {operator} {sequence}"));
    lines.push("}".to_string());

    Ok(lines.join("\n"))
}

fn validate_summary(summary: &DecodedModuleSummary) -> Result<(), RustBackendError> {
    if summary.defined_function_bodies != summary.function_bodies.len() {
        return Err(RustBackendError::InvalidInput {
            field: "decoded_module_summary.defined_function_bodies",
            message: "count mismatch with function_bodies length".to_string(),
        });
    }

    for import in &summary.import_functions {
        if import.module.trim().is_empty() {
            return Err(RustBackendError::InvalidInput {
                field: "decoded_module_summary.import_functions.module",
                message: "module must not be empty".to_string(),
            });
        }
        if import.name.trim().is_empty() {
            return Err(RustBackendError::InvalidInput {
                field: "decoded_module_summary.import_functions.name",
                message: "name must not be empty".to_string(),
            });
        }
    }

    for export in &summary.exports {
        if export.name.trim().is_empty() {
            return Err(RustBackendError::InvalidInput {
                field: "decoded_module_summary.exports.name",
                message: "name must not be empty".to_string(),
            });
        }
    }

    for body in &summary.function_bodies {
        if body.symbol.trim().is_empty() {
            return Err(RustBackendError::InvalidInput {
                field: "decoded_module_summary.function_bodies.symbol",
                message: "symbol must not be empty".to_string(),
            });
        }
    }

    Ok(())
}

fn inferred_fixture_operator(summary: &DecodedModuleSummary) -> &'static str {
    let mut saw_add = false;
    let mut saw_sub = false;
    let mut saw_mul = false;

    for body in &summary.function_bodies {
        for opcode in &body.opcodes {
            match opcode.as_str() {
                "i32.add" => saw_add = true,
                "i32.sub" => saw_sub = true,
                "i32.mul" => saw_mul = true,
                "i64.add" => saw_add = true,
                "i64.sub" => saw_sub = true,
                "i64.mul" => saw_mul = true,
                _ => {}
            }
        }
    }

    if saw_add {
        "+"
    } else if saw_sub && !saw_mul {
        "-"
    } else if saw_mul && !saw_sub {
        "*"
    } else {
        "+"
    }
}

fn render_import_line(import: ImportFunction, ident: String) -> String {
    let params = import
        .params
        .iter()
        .enumerate()
        .map(|(idx, ty)| format!("arg{idx}: {}", wasm_type_to_rust(ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = render_return_type(&import.results);

    if params.is_empty() {
        format!("        fn {ident}(){return_type};")
    } else {
        format!("        fn {ident}({params}){return_type};")
    }
}

fn render_export_comment_line(export: Export) -> String {
    format!(
        "    // export {} [{}]",
        sanitize_ident(&export.name),
        export_kind_name(export.kind)
    )
}

fn render_function_block(body: FunctionBodySummary, function_name: String) -> [String; 4] {
    let opcode_comment = if body.opcodes.is_empty() {
        "<none>".to_string()
    } else {
        body.opcodes.join(", ")
    };

    [
        format!("    pub fn {function_name}() {{"),
        format!("        // opcodes: {opcode_comment}"),
        "    }".to_string(),
        "".to_string(),
    ]
}

fn render_return_type(results: &[String]) -> String {
    match results.len() {
        0 => String::new(),
        1 => format!(" -> {}", wasm_type_to_rust(&results[0])),
        _ => format!(
            " -> ({})",
            results
                .iter()
                .map(|ty| wasm_type_to_rust(ty))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn wasm_type_to_rust(value: &str) -> &'static str {
    match value {
        "i32" => "i32",
        "i64" => "i64",
        "f32" => "f32",
        "f64" => "f64",
        _ => "i64",
    }
}

fn soroban_type_to_rust(value: &str) -> &'static str {
    let normalized = value.trim();
    match normalized {
        "i32" | "U32Val" | "u32" => "i32",
        "i64" | "u64" | "U64Val" => "i64",
        "i128" | "u128" => "i128",
        "bool" | "Bool" => "bool",
        "f32" => "f32",
        "f64" => "f64",
        "()" | "Void" => "()",
        _ if normalized.starts_with("Result") => "i64",
        _ if normalized.ends_with("Error") => "i32",
        _ if normalized.contains("Address") => "i64",
        _ if normalized.contains("Object") => "i64",
        _ if normalized.contains("Val") => "i64",
        _ => "i64",
    }
}

fn export_kind_name(kind: ExportKind) -> &'static str {
    match kind {
        ExportKind::Function => "function",
        ExportKind::Memory => "memory",
        ExportKind::Table => "table",
        ExportKind::Global => "global",
    }
}

fn sanitize_ident(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch.to_ascii_lowercase());
        } else {
            output.push('_');
        }
    }

    if output.is_empty() {
        output = "f_symbol".to_string();
    }
    if output
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        output = format!("f_{output}");
    }
    if is_rust_keyword(&output) {
        output = format!("r_{output}");
    }
    output
}

fn sanitize_type_name(input: &str) -> String {
    let ident = sanitize_ident(input);
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in ident.chars() {
        if ch == '_' {
            uppercase_next = true;
            continue;
        }
        if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "GeneratedType".to_string()
    } else if out
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        format!("T{out}")
    } else {
        out
    }
}

fn make_unique_ident(base: String, counts: &mut BTreeMap<String, usize>) -> String {
    match counts.get_mut(&base) {
        Some(next_suffix) => {
            let ident = format!("{base}_{next_suffix}");
            *next_suffix += 1;
            ident
        }
        None => {
            counts.insert(base.clone(), 1);
            base
        }
    }
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sorcat_core::{
        DecodedModuleSummary, Export, ExportKind, FunctionBodySummary, ImportFunction,
    };

    use super::{
        reconstruct_fixture_module, reconstruct_fixture_module_from_wasm, reconstruct_module,
        reconstruct_module_from_wasm, RustBackendError,
    };

    #[test]
    fn reconstruct_from_wasm_is_deterministic_for_fixture() {
        let wasm = load_wasm_fixture("sections_imports_exports.wasm");

        let first = reconstruct_module_from_wasm(&wasm)
            .expect("reconstruction should succeed for deterministic fixture");
        let second = reconstruct_module_from_wasm(&wasm)
            .expect("reconstruction should succeed for deterministic fixture");

        assert_eq!(first, second, "reconstruction output must be deterministic");
        assert!(
            first.contains("extern \"C\""),
            "expected import block in pseudo-rust output"
        );
        assert!(
            first.contains("pub fn adder(arg0: i32, arg1: i32) -> i32 {"),
            "expected typed adder function signature in reconstructed rust output"
        );
        assert!(
            first.contains("arg0 + arg1"),
            "expected executable arithmetic expression in adder body"
        );
        assert!(
            first.contains("pub fn call_log() {"),
            "expected call_log function in reconstructed rust output"
        );
        assert!(
            first.contains("host_env_log("),
            "expected host wrapper expression in call_log body"
        );
        assert!(
            !first.contains("// opcodes:"),
            "expected instruction-driven bodies instead of opcode-only comments"
        );
    }

    #[test]
    fn reconstruction_sorts_unsorted_inputs() {
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
                    results: vec![],
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

        let rendered = reconstruct_module(&summary).expect("direct summary rendering should work");
        let lines: Vec<&str> = rendered.lines().collect();
        let first_import = lines
            .iter()
            .position(|line| line.contains("fn env_a()"))
            .expect("missing env_a import");
        let second_import = lines
            .iter()
            .position(|line| line.contains("fn env_z("))
            .expect("missing env_z import");

        assert!(
            first_import < second_import,
            "imports must be rendered in deterministic order"
        );
    }

    #[test]
    fn reconstruction_rejects_invalid_summary() {
        let summary = DecodedModuleSummary {
            import_functions: vec![],
            exports: vec![],
            defined_function_bodies: 1,
            function_bodies: vec![],
        };

        let error =
            reconstruct_module(&summary).expect_err("invalid summary should produce an error");
        assert!(
            matches!(error, RustBackendError::InvalidInput { .. }),
            "expected InvalidInput for mismatched function counts"
        );
    }

    #[test]
    fn fixture_reconstruction_uses_contract_metadata_template() {
        let wasm = load_wasm_fixture("ssa_sequences.wasm");
        let rendered = reconstruct_fixture_module_from_wasm(&wasm, "synthetic/sample_v1", 21)
            .expect("fixture reconstruction should succeed for valid wasm");

        assert!(rendered.contains("#![allow(dead_code)]"));
        assert!(rendered.contains("\"synthetic/sample_v1\""));
        assert!(rendered.contains("input + 21"));
    }

    #[test]
    fn fixture_reconstruction_rejects_empty_contract_id() {
        let summary = DecodedModuleSummary {
            import_functions: vec![],
            exports: vec![],
            defined_function_bodies: 1,
            function_bodies: vec![FunctionBodySummary {
                function_index: 0,
                symbol: "f".to_string(),
                opcodes: vec!["end".to_string()],
                instructions: vec![],
            }],
        };

        let error = reconstruct_fixture_module(&summary, "   ", 1)
            .expect_err("empty contract ids should be rejected");
        assert!(
            matches!(
                error,
                RustBackendError::InvalidInput {
                    field: "contract_id",
                    ..
                }
            ),
            "expected contract_id invalid input error"
        );
    }

    #[test]
    fn reconstruction_disambiguates_colliding_identifiers() {
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
            exports: vec![],
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

        let rendered = reconstruct_module(&summary).expect("reconstruction should succeed");
        assert!(
            rendered.contains("fn env_host_fn();"),
            "first colliding import should keep base identifier"
        );
        assert!(
            rendered.contains("fn env_host_fn_1();"),
            "second colliding import should receive deterministic suffix"
        );
        assert!(
            rendered.contains("pub fn fn_a() {"),
            "first colliding function name should keep base identifier"
        );
        assert!(
            rendered.contains("pub fn fn_a_1() {"),
            "second colliding function name should receive deterministic suffix"
        );
    }

    #[test]
    fn fixture_reconstruction_infers_i64_operator() {
        let summary = DecodedModuleSummary {
            import_functions: vec![],
            exports: vec![],
            defined_function_bodies: 1,
            function_bodies: vec![FunctionBodySummary {
                function_index: 1,
                symbol: "f".to_string(),
                opcodes: vec!["i64.mul".to_string(), "end".to_string()],
                instructions: vec![],
            }],
        };

        let rendered = reconstruct_fixture_module(&summary, "synthetic/sample_v1", 5)
            .expect("fixture reconstruction should succeed");
        assert!(
            rendered.contains("input * 5"),
            "i64 opcode sequences should influence inferred operator"
        );
    }

    #[test]
    fn reconstruction_avoids_rust_keyword_identifiers() {
        let summary = DecodedModuleSummary {
            import_functions: vec![],
            exports: vec![],
            defined_function_bodies: 1,
            function_bodies: vec![FunctionBodySummary {
                function_index: 1,
                symbol: "type".to_string(),
                opcodes: vec!["end".to_string()],
                instructions: vec![],
            }],
        };

        let rendered = reconstruct_module(&summary).expect("reconstruction should succeed");
        assert!(
            rendered.contains("pub fn r_type()"),
            "keyword-like symbols should be prefixed to remain valid Rust identifiers"
        );
    }

    #[test]
    fn reconstruction_emits_typed_artifacts_from_soroban_custom_sections() {
        let wasm = wasm_with_soroban_custom_sections();

        let rendered = reconstruct_module_from_wasm(&wasm)
            .expect("reconstruction should decode and render soroban custom section semantics");

        assert!(
            rendered.contains("pub struct Allowance"),
            "decoded spec structs should be rendered as typed Rust artifacts"
        );
        assert!(
            rendered.contains("pub enum Role"),
            "decoded spec enums should be rendered as typed Rust artifacts"
        );
        assert!(
            rendered.contains("pub enum ContractError"),
            "decoded contract errors should be rendered as typed Rust artifacts"
        );
        assert!(
            rendered.contains("soroban.contract_name=token"),
            "decoded contract metadata should be reflected in reconstruction output"
        );
    }

    #[test]
    fn reconstruction_uses_explicit_wrappers_for_known_soroban_builtins() {
        let wasm = load_wasm_fixture("soroban_env_imports.wasm");

        let rendered = reconstruct_module_from_wasm(&wasm)
            .expect("reconstruction should render explicit wrappers for known builtins");

        assert!(
            rendered.contains("soroban_builtin="),
            "known soroban builtins should include canonical builtin wrapper annotations"
        );
        assert!(
            rendered.contains("pub fn host_"),
            "known soroban builtins should be exposed through deterministic wrapper functions"
        );
    }

    #[test]
    fn unsupported_constructs_are_rendered_with_safe_fallback_comments() {
        let wasm = load_wasm_fixture("unsupported_call_indirect.wasm");

        let rendered = reconstruct_module_from_wasm(&wasm)
            .expect("unsupported constructs should not crash reconstruction");

        assert!(
            rendered.contains("unsupported instruction: call_indirect"),
            "unsupported constructs should be represented with explanatory fallback comments"
        );
        assert!(
            rendered.contains("pub fn dispatch"),
            "reconstruction should still emit the surrounding function shell"
        );
    }

    #[test]
    fn reconstruction_emits_structured_control_flow_for_cfg_fixture() {
        let wasm = load_wasm_fixture("cfg_branch_loop_merge.wasm");

        let rendered = reconstruct_module_from_wasm(&wasm)
            .expect("cfg fixture should reconstruct with structured control-flow");

        assert!(
            rendered.contains("loop {"),
            "loop opcode should be reconstructed into a Rust loop block"
        );
        assert!(
            rendered.contains("if (") && rendered.contains("} else {"),
            "if/else opcodes should be reconstructed into Rust conditionals"
        );
        assert!(
            !rendered.contains("unsupported instruction: loop (structural control)"),
            "loop should no longer emit structural fallback comments"
        );
        assert!(
            !rendered.contains("unsupported instruction: if (structural control)"),
            "if should no longer emit structural fallback comments"
        );
        assert!(
            !rendered.contains("unsupported instruction: else (structural control)"),
            "else should no longer emit structural fallback comments"
        );
    }

    #[test]
    fn reconstruction_emits_match_for_br_table_when_targets_are_structured() {
        let wasm = wasm_with_single_exported_function(&[
            0x02, 0x40, // block
            0x03, 0x40, // loop
            0x41, 0x00, // i32.const 0 (selector)
            0x0e, 0x01, 0x00, 0x01, // br_table [0] default 1
            0x0b, // end loop
            0x0b, // end block
            0x0b, // end function
        ]);

        let rendered = reconstruct_module_from_wasm(&wasm)
            .expect("br_table fixture should reconstruct into match shape");

        assert!(
            rendered.contains("match "),
            "br_table should reconstruct to a Rust match expression where feasible"
        );
        assert!(
            rendered.contains("continue 'cf_"),
            "br_table loop target should map to labeled continue"
        );
        assert!(
            rendered.contains("break 'cf_"),
            "br_table block target should map to labeled break"
        );
        assert!(
            !rendered.contains("unsupported instruction: br_table"),
            "br_table should not degrade to a single fallback comment for structured targets"
        );
    }

    #[test]
    fn control_flow_reconstruction_remains_deterministic_for_cfg_fixture() {
        let wasm = load_wasm_fixture("cfg_branch_loop_merge.wasm");

        let first = reconstruct_module_from_wasm(&wasm)
            .expect("cfg fixture should reconstruct deterministically");
        let second = reconstruct_module_from_wasm(&wasm)
            .expect("cfg fixture should reconstruct deterministically");

        assert_eq!(
            first, second,
            "control-flow reconstruction output must be deterministic across runs"
        );
    }

    #[test]
    fn unsupported_branch_depth_degrades_with_explicit_comment() {
        let wasm = wasm_with_single_exported_function(&[
            0x02, 0x40, // block
            0x0c, 0x02, // br depth=2 (invalid relative to active stack depth)
            0x0b, // end block
            0x0b, // end function
        ]);

        let rendered = reconstruct_module_from_wasm(&wasm)
            .expect("unsupported branch targets should still reconstruct safely");

        assert!(
            rendered.contains("unsupported instruction: br depth=2 (branch depth exceeds active control nesting)"),
            "unsupported branch target should degrade with an explicit explanatory comment"
        );
        assert!(
            rendered.contains("'cf_0: {"),
            "surrounding structured control flow should still reconstruct"
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

    fn wasm_with_single_exported_function(body_ops: &[u8]) -> Vec<u8> {
        let mut body = Vec::with_capacity(body_ops.len() + 1);
        body.push(0x00); // local declaration count
        body.extend_from_slice(body_ops);

        let mut code_payload = vec![0x01]; // one function body
        push_leb_u32(
            u32::try_from(body.len()).expect("function body length should fit u32"),
            &mut code_payload,
        );
        code_payload.extend_from_slice(&body);

        let mut wasm = vec![
            0x00, 0x61, 0x73, 0x6d, // magic
            0x01, 0x00, 0x00, 0x00, // version
            0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // type section: one () -> ()
            0x03, 0x02, 0x01, 0x00, // function section
            0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00, // export "f"
            0x0a, // code section id
        ];
        push_leb_u32(
            u32::try_from(code_payload.len()).expect("code payload length should fit u32"),
            &mut wasm,
        );
        wasm.extend_from_slice(&code_payload);
        wasm
    }
}

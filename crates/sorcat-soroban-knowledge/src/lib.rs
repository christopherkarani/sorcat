use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

/// Stable Soroban semantic classes used by core import resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SorobanSymbolKind {
    EnvBuiltin,
    EnvUnknown,
    NonEnv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanFunctionSignature {
    pub params: Vec<String>,
    pub result: String,
}

/// Deterministic import resolution record.
///
/// `canonical_id` is only present when `kind == EnvBuiltin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorobanSymbolResolution {
    pub module: String,
    pub name: String,
    pub kind: SorobanSymbolKind,
    pub canonical_id: Option<String>,
    pub signature: Option<SorobanFunctionSignature>,
    pub min_protocol: Option<u32>,
    pub max_protocol: Option<u32>,
    pub confidence: u8,
    pub reason: String,
    pub semantic_tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SorobanKnowledge {
    /// wasm import module symbol -> wasm import field symbol -> candidate host functions
    by_export_symbol: BTreeMap<String, BTreeMap<String, Vec<HostFunction>>>,
    /// canonical host function name (e.g. `map_new`) -> candidate host functions
    by_function_name: BTreeMap<String, Vec<HostFunction>>,
    /// all known host module export symbols (e.g. `x`, `m`, `v`, ...)
    known_module_exports: BTreeSet<String>,
}

impl Default for SorobanKnowledge {
    fn default() -> Self {
        Self::from_embedded_packs()
    }
}

impl SorobanKnowledge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn classify_import(&self, module: &str, name: &str) -> SorobanSymbolKind {
        if module == "env" {
            if self
                .by_function_name
                .get(name)
                .is_some_and(|candidates| !candidates.is_empty())
            {
                SorobanSymbolKind::EnvBuiltin
            } else {
                SorobanSymbolKind::EnvUnknown
            }
        } else if self.known_module_exports.contains(module) {
            let Some(functions) = self.by_export_symbol.get(module) else {
                return SorobanSymbolKind::EnvUnknown;
            };
            if functions
                .get(name)
                .is_some_and(|candidates| !candidates.is_empty())
            {
                SorobanSymbolKind::EnvBuiltin
            } else {
                SorobanSymbolKind::EnvUnknown
            }
        } else {
            SorobanSymbolKind::NonEnv
        }
    }

    pub fn resolve_import(&self, module: &str, name: &str) -> SorobanSymbolResolution {
        self.resolve_import_with_protocol(module, name, None)
    }

    pub fn resolve_import_with_protocol(
        &self,
        module: &str,
        name: &str,
        protocol: Option<u32>,
    ) -> SorobanSymbolResolution {
        let kind = self.classify_import(module, name);

        let candidates = self.lookup_candidates(module, name);
        match kind {
            SorobanSymbolKind::EnvBuiltin => {
                let best = select_best_candidate(candidates.as_slice(), protocol);
                let protocol_specific = protocol
                    .map(|value| format!(" protocol={value}"))
                    .unwrap_or_default();

                let canonical_id = best.map(|entry| entry.canonical_id.clone());
                let signature = best.map(|entry| entry.signature.clone());
                let min_protocol = candidates.iter().map(|entry| entry.protocol).min();
                let max_protocol = candidates.iter().map(|entry| entry.protocol).max();

                let mut semantic_tags = BTreeSet::<String>::new();
                for candidate in &candidates {
                    for tag in &candidate.semantic_tags {
                        semantic_tags.insert(tag.clone());
                    }
                }

                let canonical_id_count = candidates
                    .iter()
                    .map(|entry| entry.canonical_id.clone())
                    .collect::<BTreeSet<_>>()
                    .len();
                let (confidence, reason) = if canonical_id_count <= 1 {
                    (
                        100,
                        format!(
                            "exact Soroban builtin resolution via {}{}",
                            if module == "env" {
                                "canonical function name"
                            } else {
                                "module/field symbol"
                            },
                            protocol_specific
                        ),
                    )
                } else {
                    (
                        70,
                        format!(
                            "ambiguous Soroban builtin candidates resolved by deterministic priority{}",
                            protocol_specific
                        ),
                    )
                };

                SorobanSymbolResolution {
                    module: module.to_string(),
                    name: name.to_string(),
                    kind,
                    canonical_id,
                    signature,
                    min_protocol,
                    max_protocol,
                    confidence,
                    reason,
                    semantic_tags: semantic_tags.into_iter().collect(),
                }
            }
            SorobanSymbolKind::EnvUnknown => {
                let reason = if module == "env" || self.known_module_exports.contains(module) {
                    "unresolved env import: symbol not present in embedded Soroban knowledge packs"
                } else {
                    "unresolved env import"
                };
                SorobanSymbolResolution {
                    module: module.to_string(),
                    name: name.to_string(),
                    kind,
                    canonical_id: None,
                    signature: None,
                    min_protocol: None,
                    max_protocol: None,
                    confidence: 25,
                    reason: reason.to_string(),
                    semantic_tags: vec!["env_unknown".to_string()],
                }
            }
            SorobanSymbolKind::NonEnv => SorobanSymbolResolution {
                module: module.to_string(),
                name: name.to_string(),
                kind,
                canonical_id: None,
                signature: None,
                min_protocol: None,
                max_protocol: None,
                confidence: 0,
                reason: "non-Soroban import module".to_string(),
                semantic_tags: vec!["non_env".to_string()],
            },
        }
    }

    pub fn resolve_imports<I, M, N>(&self, imports: I) -> Vec<SorobanSymbolResolution>
    where
        I: IntoIterator<Item = (M, N)>,
        M: Into<String>,
        N: Into<String>,
    {
        self.resolve_imports_with_protocol(imports, None)
    }

    pub fn resolve_imports_with_protocol<I, M, N>(
        &self,
        imports: I,
        protocol: Option<u32>,
    ) -> Vec<SorobanSymbolResolution>
    where
        I: IntoIterator<Item = (M, N)>,
        M: Into<String>,
        N: Into<String>,
    {
        let mut resolved: Vec<SorobanSymbolResolution> = imports
            .into_iter()
            .map(|(module, name)| {
                let module = module.into();
                let name = name.into();
                self.resolve_import_with_protocol(&module, &name, protocol)
            })
            .collect();

        resolved.sort_by(|left, right| {
            left.module
                .cmp(&right.module)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.canonical_id.cmp(&right.canonical_id))
                .then_with(|| left.min_protocol.cmp(&right.min_protocol))
                .then_with(|| left.max_protocol.cmp(&right.max_protocol))
                .then_with(|| left.confidence.cmp(&right.confidence))
                .then_with(|| left.reason.cmp(&right.reason))
                .then_with(|| left.semantic_tags.cmp(&right.semantic_tags))
        });

        resolved
    }

    fn lookup_candidates(&self, module: &str, name: &str) -> Vec<HostFunction> {
        if module == "env" {
            return self.by_function_name.get(name).cloned().unwrap_or_default();
        }

        self.by_export_symbol
            .get(module)
            .and_then(|fields| fields.get(name))
            .cloned()
            .unwrap_or_default()
    }

    fn from_embedded_packs() -> Self {
        // Prefer the newer pack when collisions exist (deterministic tie-break).
        let pack_22 = EnvJsonPack::from_embedded("22.1.0", include_str!("../data/env-22.1.0.json"));
        let pack_25 = EnvJsonPack::from_embedded("25.0.1", include_str!("../data/env-25.0.1.json"));

        let mut by_export_symbol: BTreeMap<String, BTreeMap<String, Vec<HostFunction>>> =
            BTreeMap::new();
        let mut by_function_name: BTreeMap<String, Vec<HostFunction>> = BTreeMap::new();
        let mut known_module_exports: BTreeSet<String> = BTreeSet::new();

        for pack in [&pack_22, &pack_25] {
            for host in pack.functions.values() {
                known_module_exports.insert(host.wasm_import_module.clone());
                by_export_symbol
                    .entry(host.wasm_import_module.clone())
                    .or_default()
                    .entry(host.wasm_import_field.clone())
                    .or_default()
                    .push(host.clone());

                by_function_name
                    .entry(host.canonical_name.clone())
                    .or_default()
                    .push(host.clone());
            }
        }

        dedupe_host_candidate_maps(&mut by_export_symbol);
        dedupe_host_vector_map(&mut by_function_name);

        Self {
            by_export_symbol,
            by_function_name,
            known_module_exports,
        }
    }
}

fn dedupe_host_candidate_maps(
    map: &mut BTreeMap<String, BTreeMap<String, Vec<HostFunction>>>,
) {
    for fields in map.values_mut() {
        for hosts in fields.values_mut() {
            dedupe_hosts(hosts);
        }
    }
}

fn dedupe_host_vector_map(map: &mut BTreeMap<String, Vec<HostFunction>>) {
    for hosts in map.values_mut() {
        dedupe_hosts(hosts);
    }
}

fn dedupe_hosts(hosts: &mut Vec<HostFunction>) {
    hosts.sort_by(|left, right| {
        left.protocol
            .cmp(&right.protocol)
            .then_with(|| left.canonical_id.cmp(&right.canonical_id))
            .then_with(|| left.wasm_import_module.cmp(&right.wasm_import_module))
            .then_with(|| left.wasm_import_field.cmp(&right.wasm_import_field))
    });

    let mut seen = BTreeSet::new();
    hosts.retain(|entry| {
        let key = format!(
            "{}|{}|{}|{}",
            entry.protocol, entry.canonical_id, entry.wasm_import_module, entry.wasm_import_field
        );
        seen.insert(key)
    });
}

fn select_best_candidate(
    candidates: &[HostFunction],
    protocol: Option<u32>,
) -> Option<&HostFunction> {
    if candidates.is_empty() {
        return None;
    }

    if let Some(protocol) = protocol {
        if let Some(best) = candidates
            .iter()
            .filter(|entry| entry.protocol <= protocol)
            .max_by(|left, right| {
                left.protocol
                    .cmp(&right.protocol)
                    .then_with(|| left.canonical_id.cmp(&right.canonical_id))
            })
        {
            return Some(best);
        }
    }

    candidates.iter().max_by(|left, right| {
        left.protocol
            .cmp(&right.protocol)
            .then_with(|| left.canonical_id.cmp(&right.canonical_id))
    })
}

pub fn classify_import(module: &str, name: &str) -> SorobanSymbolKind {
    SorobanKnowledge::default().classify_import(module, name)
}

pub fn resolve_imports<I, M, N>(imports: I) -> Vec<SorobanSymbolResolution>
where
    I: IntoIterator<Item = (M, N)>,
    M: Into<String>,
    N: Into<String>,
{
    SorobanKnowledge::default().resolve_imports(imports)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostFunction {
    canonical_id: String,
    canonical_name: String,
    module_name: String,
    wasm_import_module: String,
    wasm_import_field: String,
    protocol: u32,
    signature: SorobanFunctionSignature,
    semantic_tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct EnvJsonPack {
    #[allow(dead_code)]
    version_label: String,
    functions: BTreeMap<String, HostFunction>, // protocol|canonical_id -> host function
}

impl EnvJsonPack {
    fn from_embedded(version_label: &str, json: &str) -> Self {
        let parsed: EnvJson = match serde_json::from_str(json) {
            Ok(value) => value,
            Err(_error) => {
                return Self {
                    version_label: version_label.to_string(),
                    functions: BTreeMap::new(),
                };
            }
        };

        let protocol = parse_protocol(version_label);

        let mut functions = BTreeMap::new();
        for module in parsed.modules {
            for function in module.functions {
                let canonical_id = format!("{}.{}", module.name, function.name);
                let signature = SorobanFunctionSignature {
                    params: function.args.iter().map(|arg| arg.ty.clone()).collect(),
                    result: function.return_ty.clone(),
                };
                let entry = HostFunction {
                    canonical_id: canonical_id.clone(),
                    canonical_name: function.name.clone(),
                    module_name: module.name.clone(),
                    wasm_import_module: module.export.clone(),
                    wasm_import_field: function.export,
                    protocol,
                    signature: signature.clone(),
                    semantic_tags: infer_semantic_tags(
                        &module.name,
                        &function.name,
                        &signature.params,
                        &signature.result,
                    ),
                };
                functions.insert(format!("{protocol}|{canonical_id}"), entry);
            }
        }

        Self {
            version_label: version_label.to_string(),
            functions,
        }
    }
}

fn parse_protocol(version_label: &str) -> u32 {
    version_label
        .split('.')
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}

fn infer_semantic_tags(
    module_name: &str,
    function_name: &str,
    params: &[String],
    result: &str,
) -> Vec<String> {
    let mut tags = BTreeSet::new();
    tags.insert("env_builtin".to_string());

    if matches!(module_name, "int" | "map" | "vec" | "buf" | "str" | "symbol" | "address") {
        tags.insert("env_helper".to_string());
    }

    let looks_xdr = params.iter().any(|arg| looks_xdr_type(arg)) || looks_xdr_type(result);
    if looks_xdr {
        tags.insert("xdr_semantic".to_string());
    }

    if function_name.contains("obj_") || function_name.contains("xdr") {
        tags.insert("xdr_codec".to_string());
    }

    if function_name.contains("map_") {
        tags.insert("state_map".to_string());
    }
    if function_name.contains("vec_") {
        tags.insert("state_vec".to_string());
    }
    if function_name.contains("ledger") {
        tags.insert("ledger_context".to_string());
    }

    tags.into_iter().collect()
}

fn looks_xdr_type(ty: &str) -> bool {
    let upper = ty.to_ascii_uppercase();
    upper.contains("VAL")
        || upper.contains("OBJECT")
        || upper.contains("SC")
        || upper.contains("ERROR")
        || upper.contains("ADDRESS")
}

#[derive(Debug, Clone, Deserialize)]
struct EnvJson {
    modules: Vec<EnvJsonModule>,
}

#[derive(Debug, Clone, Deserialize)]
struct EnvJsonModule {
    name: String,
    export: String,
    functions: Vec<EnvJsonFunction>,
}

#[derive(Debug, Clone, Deserialize)]
struct EnvJsonFunction {
    export: String,
    name: String,
    args: Vec<EnvJsonArg>,
    #[serde(rename = "return")]
    return_ty: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EnvJsonArg {
    #[allow(dead_code)]
    name: String,
    #[serde(rename = "type")]
    ty: String,
}

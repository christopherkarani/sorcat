use std::path::{Path, PathBuf};
use std::{collections::BTreeSet, path::Component};

use serde::{Deserialize, Serialize};

use crate::EvalError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusCategory {
    RealWorld,
    Synthetic,
    Adversarial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildProfile {
    Debug,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildVariant {
    pub profile: BuildProfile,
    pub include_debug_names: bool,
    pub sdk_version: String,
    pub wasm_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusContractEntry {
    pub id: String,
    pub category: CorpusCategory,
    pub rust_source: PathBuf,
    pub metadata_path: PathBuf,
    pub variants: Vec<BuildVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusManifest {
    pub schema_version: String,
    pub locked: bool,
    pub contracts: Vec<CorpusContractEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceStatusSummary {
    pub verified_contracts: usize,
    pub pending_contract_ids: Vec<String>,
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<CorpusManifest, EvalError> {
    let path = path.as_ref().to_path_buf();
    let manifest_content = std::fs::read_to_string(&path).map_err(|source| EvalError::Io {
        path: path.clone(),
        source,
    })?;

    let manifest: CorpusManifest =
        serde_json::from_str(&manifest_content).map_err(|source| EvalError::Json {
            path: path.clone(),
            source,
        })?;

    validate_manifest_schema(&manifest)?;
    Ok(manifest)
}

pub fn validate_corpus_layout(
    corpus_root: impl AsRef<Path>,
    manifest: &CorpusManifest,
) -> Result<(), EvalError> {
    validate_manifest_schema(manifest)?;

    let corpus_root = corpus_root.as_ref();
    if !corpus_root.exists() {
        return Err(EvalError::InvalidManifest {
            message: format!("corpus root `{}` does not exist", corpus_root.display()),
        });
    }
    if !corpus_root.is_dir() {
        return Err(EvalError::InvalidManifest {
            message: format!("corpus root `{}` is not a directory", corpus_root.display()),
        });
    }

    for contract in &manifest.contracts {
        let source_path = resolve_under_root(corpus_root, &contract.rust_source, "rust_source")?;
        ensure_non_empty_file(&source_path, "rust_source", &contract.id)?;

        let metadata_path =
            resolve_under_root(corpus_root, &contract.metadata_path, "metadata_path")?;
        let metadata_json = ensure_json_file(&metadata_path, "metadata_path", &contract.id)?;
        if matches!(contract.category, CorpusCategory::RealWorld) {
            validate_real_world_source_provenance(&metadata_json, &metadata_path, &contract.id)?;
        }

        for variant in &contract.variants {
            let wasm_path = resolve_under_root(corpus_root, &variant.wasm_path, "wasm_path")?;
            ensure_wasm_binary(&wasm_path, &contract.id)?;
        }
    }

    Ok(())
}

pub fn collect_real_world_provenance_status(
    corpus_root: impl AsRef<Path>,
    manifest: &CorpusManifest,
) -> Result<ProvenanceStatusSummary, EvalError> {
    validate_manifest_schema(manifest)?;
    let corpus_root = corpus_root.as_ref();

    let mut verified_contracts = 0usize;
    let mut pending_contract_ids = Vec::new();
    for contract in &manifest.contracts {
        if !matches!(contract.category, CorpusCategory::RealWorld) {
            continue;
        }

        let metadata_path =
            resolve_under_root(corpus_root, &contract.metadata_path, "metadata_path")?;
        let metadata_json = ensure_json_file(&metadata_path, "metadata_path", &contract.id)?;
        validate_real_world_source_provenance(&metadata_json, &metadata_path, &contract.id)?;

        let verification_status = metadata_json
            .get("source_provenance")
            .and_then(|value| value.get("verification_status"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("pending")
            .trim()
            .to_ascii_lowercase();

        if verification_status == "verified" {
            verified_contracts += 1;
        } else {
            pending_contract_ids.push(contract.id.clone());
        }
    }

    pending_contract_ids.sort();
    Ok(ProvenanceStatusSummary {
        verified_contracts,
        pending_contract_ids,
    })
}

fn validate_manifest_schema(manifest: &CorpusManifest) -> Result<(), EvalError> {
    if manifest.schema_version != "1.0.0" {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "unsupported schema_version `{}`; expected `1.0.0`",
                manifest.schema_version
            ),
        });
    }

    if manifest.contracts.is_empty() {
        return Err(EvalError::InvalidManifest {
            message: "manifest must contain at least one contract".to_string(),
        });
    }

    let mut contract_ids = BTreeSet::new();
    for (contract_idx, contract) in manifest.contracts.iter().enumerate() {
        if contract.id.trim().is_empty() {
            return Err(EvalError::InvalidManifest {
                message: format!("contracts[{contract_idx}].id cannot be empty"),
            });
        }

        if !contract_ids.insert(contract.id.clone()) {
            return Err(EvalError::InvalidManifest {
                message: format!("duplicate contract id `{}`", contract.id),
            });
        }

        validate_manifest_relative_path(
            &contract.rust_source,
            &format!("contracts[{contract_idx}].rust_source"),
        )?;
        if contract
            .rust_source
            .extension()
            .and_then(|value| value.to_str())
            != Some("rs")
        {
            return Err(EvalError::InvalidManifest {
                message: format!("contracts[{contract_idx}].rust_source must end with `.rs`"),
            });
        }

        validate_manifest_relative_path(
            &contract.metadata_path,
            &format!("contracts[{contract_idx}].metadata_path"),
        )?;
        if contract
            .metadata_path
            .extension()
            .and_then(|value| value.to_str())
            != Some("json")
        {
            return Err(EvalError::InvalidManifest {
                message: format!("contracts[{contract_idx}].metadata_path must end with `.json`"),
            });
        }

        if contract.variants.is_empty() {
            return Err(EvalError::InvalidManifest {
                message: format!(
                    "contract `{}` must define at least one variant",
                    contract.id
                ),
            });
        }

        let mut variant_keys = BTreeSet::new();
        for (variant_idx, variant) in contract.variants.iter().enumerate() {
            if variant.sdk_version.trim().is_empty() {
                return Err(EvalError::InvalidManifest {
                    message: format!(
                        "contracts[{contract_idx}].variants[{variant_idx}].sdk_version cannot be empty"
                    ),
                });
            }

            validate_manifest_relative_path(
                &variant.wasm_path,
                &format!("contracts[{contract_idx}].variants[{variant_idx}].wasm_path"),
            )?;
            if variant
                .wasm_path
                .extension()
                .and_then(|value| value.to_str())
                != Some("wasm")
            {
                return Err(EvalError::InvalidManifest {
                    message: format!(
                        "contracts[{contract_idx}].variants[{variant_idx}].wasm_path must end with `.wasm`"
                    ),
                });
            }

            let variant_key = format!(
                "{:?}|{}|{}|{}",
                variant.profile,
                variant.include_debug_names,
                variant.sdk_version,
                variant.wasm_path.display()
            );
            if !variant_keys.insert(variant_key) {
                return Err(EvalError::InvalidManifest {
                    message: format!(
                        "contract `{}` contains duplicate variant at index {}",
                        contract.id, variant_idx
                    ),
                });
            }
        }
    }

    Ok(())
}

fn validate_manifest_relative_path(path: &Path, field: &str) -> Result<(), EvalError> {
    if path.as_os_str().is_empty() {
        return Err(EvalError::InvalidManifest {
            message: format!("{field} cannot be empty"),
        });
    }

    if path.is_absolute() {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "{field} must be relative, found absolute path `{}`",
                path.display()
            ),
        });
    }

    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(EvalError::InvalidManifest {
                    message: format!(
                        "{field} must not contain traversal or root components: `{}`",
                        path.display()
                    ),
                });
            }
        }
    }

    Ok(())
}

fn resolve_under_root(root: &Path, relative: &Path, field: &str) -> Result<PathBuf, EvalError> {
    validate_manifest_relative_path(relative, field)?;
    Ok(root.join(relative))
}

fn ensure_non_empty_file(path: &Path, field: &str, contract_id: &str) -> Result<(), EvalError> {
    let contents = std::fs::read(path).map_err(|source| EvalError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if contents.is_empty() {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "contract `{contract_id}` has empty {field} file at `{}`",
                path.display()
            ),
        });
    }
    Ok(())
}

fn ensure_json_file(
    path: &Path,
    field: &str,
    contract_id: &str,
) -> Result<serde_json::Value, EvalError> {
    let text = std::fs::read_to_string(path).map_err(|source| EvalError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if text.trim().is_empty() {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "contract `{contract_id}` has empty {field} file at `{}`",
                path.display()
            ),
        });
    }

    serde_json::from_str::<serde_json::Value>(&text).map_err(|source| EvalError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn ensure_wasm_binary(path: &Path, contract_id: &str) -> Result<(), EvalError> {
    let bytes = std::fs::read(path).map_err(|source| EvalError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    if bytes.len() < 8 {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "contract `{contract_id}` wasm `{}` is too short to contain wasm header",
                path.display()
            ),
        });
    }

    const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
    const WASM_VERSION_1: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

    if bytes[..4] != WASM_MAGIC {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "contract `{contract_id}` wasm `{}` has invalid wasm magic header",
                path.display()
            ),
        });
    }

    if bytes[4..8] != WASM_VERSION_1 {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "contract `{contract_id}` wasm `{}` has unsupported wasm version bytes",
                path.display()
            ),
        });
    }

    Ok(())
}

fn validate_real_world_source_provenance(
    metadata_json: &serde_json::Value,
    metadata_path: &Path,
    contract_id: &str,
) -> Result<(), EvalError> {
    let metadata_obj = metadata_json.as_object().ok_or_else(|| EvalError::InvalidManifest {
        message: format!(
            "contract `{contract_id}` metadata `{}` must be a JSON object",
            metadata_path.display()
        ),
    })?;

    let provenance = metadata_obj
        .get("source_provenance")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| EvalError::InvalidManifest {
            message: format!(
                "contract `{contract_id}` metadata `{}` must include `source_provenance` object",
                metadata_path.display()
            ),
        })?;

    for field in [
        "upstream_repo_url",
        "upstream_commit",
        "upstream_license",
        "source_origin",
        "build_recipe",
    ] {
        let value = provenance
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim();
        if value.is_empty() {
            return Err(EvalError::InvalidManifest {
                message: format!(
                    "contract `{contract_id}` provenance field `{field}` must be non-empty: `{}`",
                    metadata_path.display()
                ),
            });
        }
        if looks_placeholder_value(value) {
            return Err(EvalError::InvalidManifest {
                message: format!(
                    "contract `{contract_id}` provenance field `{field}` uses placeholder-like value `{value}`",
                ),
            });
        }
    }

    let upstream_repo_url = provenance
        .get("upstream_repo_url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    if !upstream_repo_url.starts_with("https://") {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "contract `{contract_id}` provenance `upstream_repo_url` must start with https://",
            ),
        });
    }

    let upstream_commit = provenance
        .get("upstream_commit")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    let commit_hex_like = upstream_commit.len() >= 7
        && upstream_commit
            .chars()
            .all(|ch| ch.is_ascii_hexdigit());
    if !commit_hex_like {
        return Err(EvalError::InvalidManifest {
            message: format!(
                "contract `{contract_id}` provenance `upstream_commit` must look like a real commit hash",
            ),
        });
    }

    Ok(())
}

fn looks_placeholder_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("example.invalid")
        || lower.contains("example.com")
        || lower.contains("placeholder")
        || lower.contains("todo")
        || lower.contains("tbd")
        || lower.contains("locked-corpus-v1-seq")
        || lower.contains("curated_fixture_seed")
}

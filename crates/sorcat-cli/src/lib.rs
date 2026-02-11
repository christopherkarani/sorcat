use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::BTreeSet;

use clap::{Args, Parser, Subcommand, ValueEnum};
use indexmap::IndexMap;
use sorcat_core::{
    CoreError, CoreErrorKind, EdgeKind, ParseLimits, SorobanImportKind,
    build_cfg_summary_with_limits, decode_module_summary_with_limits,
    decode_soroban_custom_sections_with_limits, lift_function_to_ssa_summary_with_limits,
    resolve_soroban_imports_with_limits, summarize_export_opcodes_with_limits,
};
use sorcat_eval::{
    ast::{normalize_original_rust, normalize_reconstructed_rust, NormalizationOptions},
    corpus::{
        BuildProfile, BuildVariant, collect_real_world_provenance_status, load_manifest,
        validate_corpus_layout,
    },
    report::{render_deterministic_report, DeterministicReport},
    scoring::{compute_ast_score, evaluate_thresholds, summarize, ContractScore, Thresholds},
    EvalError,
};
use sorcat_rust_backend::{
    reconstruct_module_from_wasm, RustBackendError,
};
use sorcat_wat_backend::{
    render_module_summary_from_wasm,
    render_wat_from_wasm_with_soroban_annotations,
    WatBackendError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("failed to read `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("core analysis failed ({kind:?}): {message}")]
    Core {
        kind: CoreErrorKind,
        message: String,
    },
    #[error("WAT backend error: {0}")]
    WatBackend(#[from] WatBackendError),
    #[error("Rust backend error: {0}")]
    RustBackend(#[from] RustBackendError),
    #[error("evaluation error: {0}")]
    Eval(#[from] EvalError),
    #[error("invalid argument `{field}`: {message}")]
    InvalidArgument {
        field: &'static str,
        message: String,
    },
}

impl From<CoreError> for CliError {
    fn from(value: CoreError) -> Self {
        Self::Core {
            kind: value.kind,
            message: value.message,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "sorcat-cli",
    version,
    about = "Deterministic baseline Soroban WASM decompilation and scoring CLI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Decompile a WASM module into deterministic WAT/Rust summaries
    Decompile(DecompileArgs),
    /// Run deterministic baseline scoring with sorcat-eval manifest/report APIs
    Score(ScoreArgs),
    /// Explain decode, CFG, SSA, and Soroban import info for one export
    Explain(ExplainArgs),
    /// Deterministically diff two WASM summaries
    Diff(DiffArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DecompileBackend {
    Wat,
    Rust,
    Both,
}

#[derive(Debug, Args)]
pub struct DecompileArgs {
    /// Path to input WASM module
    pub wasm_path: PathBuf,
    /// Output backend selection
    #[arg(long, value_enum, default_value_t = DecompileBackend::Both)]
    pub backend: DecompileBackend,
    #[command(flatten)]
    pub limits: ParseLimitArgs,
}

#[derive(Debug, Args)]
pub struct ExplainArgs {
    /// Path to input WASM module
    pub wasm_path: PathBuf,
    /// Export name to explain
    pub export: String,
    #[command(flatten)]
    pub limits: ParseLimitArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DiffFormat {
    Wat,
    Rust,
}

#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Left-hand WASM module path
    pub left_wasm_path: PathBuf,
    /// Right-hand WASM module path
    pub right_wasm_path: PathBuf,
    /// Summary format used for comparison
    #[arg(long, value_enum, default_value_t = DiffFormat::Wat)]
    pub format: DiffFormat,
    #[command(flatten)]
    pub limits: ParseLimitArgs,
}

#[derive(Debug, Args)]
pub struct ScoreArgs {
    /// Path to corpus manifest JSON
    #[arg(long, default_value = "fixtures/corpus/manifest.v1.json")]
    pub manifest: PathBuf,
    /// Root path that contains manifest relative corpus entries
    #[arg(long, default_value = "fixtures/corpus")]
    pub corpus_root: PathBuf,
    /// Optional file path to write the deterministic JSON report
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Minimum mean AST score threshold
    #[arg(long, default_value_t = 0.90)]
    pub min_mean_ast_score: f64,
    /// Minimum builtin coverage threshold
    #[arg(long, default_value_t = 0.98)]
    pub min_builtin_coverage: f64,
    /// Require all real_world corpus entries to have verified provenance metadata.
    #[arg(long, default_value_t = false)]
    pub require_submission_ready: bool,
    #[command(flatten)]
    pub limits: ParseLimitArgs,
}

#[derive(Debug, Clone, Args)]
pub struct ParseLimitArgs {
    /// Maximum accepted WASM input size in bytes.
    #[arg(long, default_value_t = 16 * 1024 * 1024)]
    pub max_wasm_bytes: usize,
    /// Maximum decoded instructions allowed per function body.
    #[arg(long, default_value_t = 250_000)]
    pub max_instructions_per_function: usize,
    /// Maximum structured block nesting depth.
    #[arg(long, default_value_t = 4_096)]
    pub max_block_nesting_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScoreContractMetadata {
    id: String,
    sequence: u64,
}

pub fn run_from<I, T>(args: I) -> Result<String, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    run(cli)
}

pub fn run(cli: Cli) -> Result<String, CliError> {
    match cli.command {
        CliCommand::Decompile(args) => run_decompile(args),
        CliCommand::Score(args) => run_score(args),
        CliCommand::Explain(args) => run_explain(args),
        CliCommand::Diff(args) => run_diff(args),
    }
}

fn run_decompile(args: DecompileArgs) -> Result<String, CliError> {
    let wasm = read_wasm(&args.wasm_path)?;
    let limits = parse_limits(&args.limits)?;
    enforce_wasm_limits(&wasm, &limits)?;
    let mut sections = Vec::new();

    match args.backend {
        DecompileBackend::Wat => {
            let rendered = render_wat_from_wasm_with_soroban_annotations(&wasm)?;
            sections.push(format!("== WAT ==\n{rendered}"));
        }
        DecompileBackend::Rust => {
            let rendered = reconstruct_module_from_wasm(&wasm)?;
            sections.push(format!("== RUST SUMMARY ==\n{rendered}"));
        }
        DecompileBackend::Both => {
            let wat = render_wat_from_wasm_with_soroban_annotations(&wasm)?;
            let rust = reconstruct_module_from_wasm(&wasm)?;
            sections.push(format!("== WAT ==\n{wat}"));
            sections.push(format!("== RUST SUMMARY ==\n{rust}"));
        }
    }

    Ok(sections.join("\n\n"))
}

fn run_explain(args: ExplainArgs) -> Result<String, CliError> {
    if args.export.trim().is_empty() {
        return Err(CliError::InvalidArgument {
            field: "export",
            message: "must not be empty".to_string(),
        });
    }

    let limits = parse_limits(&args.limits)?;
    let wasm = read_wasm(&args.wasm_path)?;
    let decoded = decode_module_summary_with_limits(&wasm, &limits)?;
    let cfg = build_cfg_summary_with_limits(&wasm, &args.export, &limits)?;
    let ssa = lift_function_to_ssa_summary_with_limits(&wasm, &args.export, &limits)?;
    let imports = resolve_soroban_imports_with_limits(&wasm, &limits)?;

    let mut edges = cfg.edges.clone();
    edges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then_with(|| left.to.cmp(&right.to))
            .then_with(|| edge_kind_text(left.kind).cmp(edge_kind_text(right.kind)))
    });

    let mut exits = cfg.exits.clone();
    exits.sort();

    let mut function_symbols = decoded
        .function_bodies
        .iter()
        .map(|body| body.symbol.clone())
        .collect::<Vec<_>>();
    function_symbols.sort();

    let selected_body = summarize_export_opcodes_with_limits(&wasm, &args.export, &limits)?
        .join(", ");

    let mut lines = vec![
        format!("explain export={}", args.export),
        format!("decode.import_functions={}", decoded.import_functions.len()),
        format!("decode.exports={}", decoded.exports.len()),
        format!(
            "decode.defined_function_bodies={}",
            decoded.defined_function_bodies
        ),
        format!("decode.function_symbols={}", function_symbols.join(", ")),
        format!("decode.selected_export_opcodes={selected_body}"),
        "imports:".to_string(),
    ];

    for import in imports {
        lines.push(format!(
            "  - {}::{} [{}]",
            import.module,
            import.name,
            import_kind_text(import.kind)
        ));
    }

    lines.push("cfg:".to_string());
    lines.push(format!("  entry={}", cfg.entry));
    lines.push(format!("  blocks={}", cfg.blocks.join(", ")));
    for edge in edges {
        lines.push(format!(
            "  edge {} -> {} ({})",
            edge.from,
            edge.to,
            edge_kind_text(edge.kind)
        ));
    }
    lines.push(format!("  exits={}", exits.join(", ")));

    lines.push("ssa:".to_string());
    lines.push(format!("  params={}", ssa.params.join(", ")));
    lines.push(format!("  instructions={}", ssa.instructions.join(", ")));
    lines.push(format!("  phi_nodes={}", ssa.phi_nodes));
    lines.push(format!("  terminator={}", ssa.terminator));

    Ok(lines.join("\n"))
}

fn run_diff(args: DiffArgs) -> Result<String, CliError> {
    let left_wasm = read_wasm(&args.left_wasm_path)?;
    let right_wasm = read_wasm(&args.right_wasm_path)?;
    let limits = parse_limits(&args.limits)?;
    enforce_wasm_limits(&left_wasm, &limits)?;
    enforce_wasm_limits(&right_wasm, &limits)?;

    let (left, right, format_label) = match args.format {
        DiffFormat::Wat => (
            render_module_summary_from_wasm(&left_wasm)?,
            render_module_summary_from_wasm(&right_wasm)?,
            "wat",
        ),
        DiffFormat::Rust => (
            reconstruct_module_from_wasm(&left_wasm)?,
            reconstruct_module_from_wasm(&right_wasm)?,
            "rust",
        ),
    };

    if left == right {
        return Ok(format!(
            "no differences detected for {} summaries",
            format_label
        ));
    }

    let diff_lines = deterministic_line_diff(&left, &right);
    let mut output = vec![
        format!("diff format={format_label}"),
        format!("--- {}", args.left_wasm_path.display()),
        format!("+++ {}", args.right_wasm_path.display()),
    ];
    output.extend(diff_lines);
    Ok(output.join("\n"))
}

fn run_score(args: ScoreArgs) -> Result<String, CliError> {
    validate_threshold_argument("min_mean_ast_score", args.min_mean_ast_score)?;
    validate_threshold_argument("min_builtin_coverage", args.min_builtin_coverage)?;
    let limits = parse_limits(&args.limits)?;

    let manifest = load_manifest(&args.manifest)?;
    validate_corpus_layout(&args.corpus_root, &manifest)?;

    const LOCKED_MIN_MEAN_AST_SCORE: f64 = 0.90;
    const LOCKED_MIN_BUILTIN_COVERAGE: f64 = 0.98;
    if manifest.locked {
        if args.min_mean_ast_score < LOCKED_MIN_MEAN_AST_SCORE {
            return Err(CliError::InvalidArgument {
                field: "min_mean_ast_score",
                message: format!(
                    "locked corpus requires threshold >= {LOCKED_MIN_MEAN_AST_SCORE:.2}"
                ),
            });
        }
        if args.min_builtin_coverage < LOCKED_MIN_BUILTIN_COVERAGE {
            return Err(CliError::InvalidArgument {
                field: "min_builtin_coverage",
                message: format!(
                    "locked corpus requires threshold >= {LOCKED_MIN_BUILTIN_COVERAGE:.2}"
                ),
            });
        }
    }

    let normalization_options = NormalizationOptions::default();
    let mut contracts = manifest.contracts.clone();
    contracts.sort_by(|left, right| left.id.cmp(&right.id));
    let mut locked_sequences = BTreeSet::new();

    let mut contract_scores = Vec::with_capacity(contracts.len());
    for contract in &contracts {
        let source_path = args.corpus_root.join(&contract.rust_source);
        let source = read_text(&source_path)?;
        let metadata_path = args.corpus_root.join(&contract.metadata_path);
        let score_metadata = read_score_metadata(&metadata_path, &contract.id, manifest.locked)?;
        if manifest.locked {
            let metadata = score_metadata.as_ref().ok_or(CliError::InvalidArgument {
                field: "metadata_path",
                message: format!(
                    "locked corpus contract `{}` must include aligned score metadata",
                    contract.id
                ),
            })?;
            if !locked_sequences.insert(metadata.sequence) {
                return Err(CliError::InvalidArgument {
                    field: "metadata_path.sequence",
                    message: format!(
                        "duplicate metadata sequence value `{}` detected in locked corpus",
                        metadata.sequence
                    ),
                });
            }
        }

        let normalized_original = normalize_original_rust(
            &normalize_source_for_scoring(&source),
            &normalization_options,
        )?;
        let mut variants = contract.variants.clone();
        sort_variants(&mut variants);
        if variants.is_empty() {
            return Err(CliError::InvalidArgument {
                field: "manifest.contracts.variants",
                message: format!("contract `{}` has no variants", contract.id),
            });
        }

        for variant in &variants {
            let wasm_path = args.corpus_root.join(&variant.wasm_path);
            let wasm = read_wasm(&wasm_path)?;
            enforce_wasm_limits(&wasm, &limits)?;

            let reconstructed = reconstruct_module_from_wasm(&wasm)?;
            let normalized_reconstructed = normalize_reconstructed_rust(
                &normalize_source_for_scoring(&reconstructed),
                &normalization_options,
            )?;
            let ast_score = compute_ast_score(&normalized_original, &normalized_reconstructed)?;

            let import_resolutions = resolve_soroban_imports_with_limits(&wasm, &limits)?;
            let (builtin_hits, builtin_total) = baseline_builtin_coverage(&import_resolutions);

            contract_scores.push(ContractScore {
                contract_id: variant_contract_score_id(&contract.id, variant),
                ast_score,
                builtin_hits,
                builtin_total,
            });
        }
    }

    if manifest.locked {
        let expected: BTreeSet<u64> = (1..=contracts.len() as u64).collect();
        if locked_sequences != expected {
            return Err(CliError::InvalidArgument {
                field: "metadata_path.sequence",
                message: "locked corpus metadata sequences must be unique and contiguous".to_string(),
            });
        }
    }

    let summary = summarize(&contract_scores)?;
    let thresholds = Thresholds {
        min_mean_ast_score: args.min_mean_ast_score,
        min_builtin_coverage: args.min_builtin_coverage,
    };
    evaluate_thresholds(&summary, &thresholds)?;

    let provenance_status = collect_real_world_provenance_status(&args.corpus_root, &manifest)?;
    let submission_ready = provenance_status.pending_contract_ids.is_empty();
    if args.require_submission_ready && !submission_ready {
        return Err(CliError::InvalidArgument {
            field: "require_submission_ready",
            message: format!(
                "submission-ready blocked: {} real_world contracts are still provenance verification pending",
                provenance_status.pending_contract_ids.len()
            ),
        });
    }

    let mut metadata = IndexMap::new();
    metadata.insert("locked".to_string(), manifest.locked.to_string());
    metadata.insert(
        "contracts".to_string(),
        manifest.contracts.len().to_string(),
    );
    metadata.insert(
        "variants_scored".to_string(),
        contract_scores.len().to_string(),
    );
    if manifest.locked {
        metadata.insert(
            "metadata_alignment".to_string(),
            "locked-corpus-id-sequence".to_string(),
        );
    }
    metadata.insert(
        "provenance_verified_contracts".to_string(),
        provenance_status.verified_contracts.to_string(),
    );
    metadata.insert(
        "provenance_pending_contracts".to_string(),
        provenance_status.pending_contract_ids.len().to_string(),
    );
    metadata.insert(
        "provenance_verification_mode".to_string(),
        if submission_ready {
            "verified".to_string()
        } else {
            "pending".to_string()
        },
    );
    metadata.insert(
        "threshold_min_mean_ast_score".to_string(),
        format!("{:.4}", thresholds.min_mean_ast_score),
    );
    metadata.insert(
        "threshold_min_builtin_coverage".to_string(),
        format!("{:.4}", thresholds.min_builtin_coverage),
    );
    metadata.insert("scoring_path".to_string(), "decompile-manifest".to_string());

    let report = DeterministicReport {
        corpus_revision: format!(
            "schema:{}|contracts:{}|locked:{}",
            manifest.schema_version,
            manifest.contracts.len(),
            manifest.locked
        ),
        summary,
        metadata,
    };
    let report_json = render_deterministic_report(&report)?;

    if let Some(path) = &args.output {
        fs::write(path, &report_json).map_err(|source| CliError::Io {
            path: path.clone(),
            source,
        })?;
    }

    let mut lines = vec![
        format!("contracts_scored={}", report.summary.contract_scores.len()),
        format!("mean_ast_score={:.6}", report.summary.mean_ast_score),
        format!(
            "builtin_coverage={:.6}",
            report.summary.builtin_coverage.ratio
        ),
        format!("submission_ready={submission_ready}"),
        format!(
            "provenance_pending_contracts={}",
            provenance_status.pending_contract_ids.len()
        ),
    ];
    if let Some(path) = args.output {
        lines.push(format!("report_path={}", path.display()));
    }
    lines.push(format!("report_json={report_json}"));
    Ok(lines.join("\n"))
}

fn validate_threshold_argument(field: &'static str, value: f64) -> Result<(), CliError> {
    if !value.is_finite() {
        return Err(CliError::InvalidArgument {
            field,
            message: "must be finite".to_string(),
        });
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(CliError::InvalidArgument {
            field,
            message: "must be within [0.0, 1.0]".to_string(),
        });
    }
    Ok(())
}

fn parse_limits(args: &ParseLimitArgs) -> Result<ParseLimits, CliError> {
    if args.max_wasm_bytes == 0 {
        return Err(CliError::InvalidArgument {
            field: "max_wasm_bytes",
            message: "must be greater than zero".to_string(),
        });
    }
    if args.max_instructions_per_function == 0 {
        return Err(CliError::InvalidArgument {
            field: "max_instructions_per_function",
            message: "must be greater than zero".to_string(),
        });
    }
    if args.max_block_nesting_depth == 0 {
        return Err(CliError::InvalidArgument {
            field: "max_block_nesting_depth",
            message: "must be greater than zero".to_string(),
        });
    }
    Ok(ParseLimits {
        max_wasm_bytes: args.max_wasm_bytes,
        max_instructions_per_function: args.max_instructions_per_function,
        max_block_nesting_depth: args.max_block_nesting_depth,
    })
}

fn enforce_wasm_limits(wasm: &[u8], limits: &ParseLimits) -> Result<(), CliError> {
    decode_module_summary_with_limits(wasm, limits)?;
    decode_soroban_custom_sections_with_limits(wasm, limits)?;
    Ok(())
}

fn read_wasm(path: &Path) -> Result<Vec<u8>, CliError> {
    fs::read(path).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_text(path: &Path) -> Result<String, CliError> {
    fs::read_to_string(path).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_score_metadata(
    path: &Path,
    expected_contract_id: &str,
    require_alignment: bool,
) -> Result<Option<ScoreContractMetadata>, CliError> {
    let contents = read_text(path)?;
    let value: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(value) => value,
        Err(_error) if !require_alignment => return Ok(None),
        Err(error) => {
            return Err(CliError::InvalidArgument {
                field: "metadata_path",
                message: format!("`{}` is not valid JSON: {error}", path.display()),
            });
        }
    };

    let object = match value.as_object() {
        Some(object) => object,
        None if !require_alignment => return Ok(None),
        None => {
            return Err(CliError::InvalidArgument {
                field: "metadata_path",
                message: format!("`{}` must contain a JSON object", path.display()),
            });
        }
    };

    let Some(id_value) = object.get("id").and_then(serde_json::Value::as_str) else {
        return if require_alignment {
            Err(CliError::InvalidArgument {
                field: "metadata_path.id",
                message: format!(
                    "`{}` must include a non-empty string `id` field",
                    path.display()
                ),
            })
        } else {
            Ok(None)
        };
    };
    let id = id_value.trim().to_string();
    if id.is_empty() {
        return Err(CliError::InvalidArgument {
            field: "metadata_path.id",
            message: format!("`{}` contains an empty `id` field", path.display()),
        });
    }
    if id != expected_contract_id {
        return Err(CliError::InvalidArgument {
            field: "metadata_path.id",
            message: format!(
                "metadata id `{id}` does not match manifest contract id `{expected_contract_id}`",
            ),
        });
    }

    let Some(sequence) = object.get("sequence").and_then(serde_json::Value::as_u64) else {
        return if require_alignment {
            Err(CliError::InvalidArgument {
                field: "metadata_path.sequence",
                message: format!(
                    "`{}` must include a positive integer `sequence` field",
                    path.display()
                ),
            })
        } else {
            Ok(None)
        };
    };
    if sequence == 0 {
        return Err(CliError::InvalidArgument {
            field: "metadata_path.sequence",
            message: format!("`{}` must include `sequence >= 1`", path.display()),
        });
    }

    Ok(Some(ScoreContractMetadata { id, sequence }))
}

fn sort_variants(variants: &mut [BuildVariant]) {
    variants.sort_by(|left, right| {
        profile_rank(&left.profile)
            .cmp(&profile_rank(&right.profile))
            .then_with(|| left.include_debug_names.cmp(&right.include_debug_names))
            .then_with(|| left.sdk_version.cmp(&right.sdk_version))
            .then_with(|| left.wasm_path.cmp(&right.wasm_path))
    });
}

fn profile_rank(profile: &BuildProfile) -> u8 {
    match profile {
        BuildProfile::Debug => 0,
        BuildProfile::Release => 1,
    }
}

fn variant_contract_score_id(contract_id: &str, variant: &BuildVariant) -> String {
    let profile = match variant.profile {
        BuildProfile::Debug => "debug",
        BuildProfile::Release => "release",
    };
    let names = if variant.include_debug_names {
        "with_names"
    } else {
        "stripped"
    };
    format!("{contract_id}::{profile}::{names}::{}", variant.sdk_version)
}

fn baseline_builtin_coverage(imports: &[sorcat_core::SorobanImportResolution]) -> (usize, usize) {
    let mut env_total = 0usize;
    let mut env_covered = 0usize;

    for entry in imports {
        match entry.kind {
            SorobanImportKind::EnvBuiltin => {
                env_total += 1;
                env_covered += 1;
            }
            SorobanImportKind::EnvUnknown => {
                env_total += 1;
            }
            SorobanImportKind::NonEnv => {}
        }
    }

    if env_total == 0 {
        (0, 0)
    } else {
        (env_covered, env_total)
    }
}

fn normalize_source_for_scoring(source: &str) -> String {
    let mut blocks = Vec::<(String, String)>::new();
    let mut lines = source.lines();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.contains("fn ") || !trimmed.contains('{') {
            continue;
        }

        let mut block_lines = vec![line.to_string()];
        let mut depth = brace_delta(line);
        while depth > 0 {
            let Some(next_line) = lines.next() else {
                break;
            };
            depth += brace_delta(next_line);
            block_lines.push(next_line.to_string());
        }

        let header = block_lines
            .first()
            .map(|entry| entry.trim())
            .unwrap_or_default();
        if header.starts_with("fn panic(") || header.contains(" fn panic(") {
            continue;
        }
        let is_public_fn = header.contains("pub fn ") || header.contains("pub extern \"C\" fn");
        if !is_public_fn {
            continue;
        }

        let normalized_block = canonicalize_function_block_for_scoring(&block_lines);
        let function_name = extract_function_name(&normalized_block).unwrap_or_default();
        if function_name.starts_with("host_") {
            continue;
        }
        blocks.push((function_name, normalized_block));
    }

    if blocks.is_empty() {
        source.to_string()
    } else {
        blocks.sort_by(|left, right| left.0.cmp(&right.0));
        format!(
            "{}\n",
            blocks
                .into_iter()
                .map(|(_, block)| block)
                .collect::<Vec<_>>()
                .join("\n\n")
        )
    }
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '{').count() as i32;
    let closes = line.chars().filter(|ch| *ch == '}').count() as i32;
    opens - closes
}

fn canonicalize_function_block_for_scoring(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let mut out = Vec::<String>::new();
    let mut header = lines[0].clone();
    header = header.replace("extern \"C\" ", "");
    header = header.replace("unsafe fn", "fn");
    out.push(header);

    let mut unsafe_wrapper_depth = 0usize;
    for (idx, line) in lines.iter().enumerate().skip(1) {
        let trimmed = line.trim();
        let is_last_line = idx + 1 == lines.len();

        if !is_last_line && trimmed == "unsafe {" {
            unsafe_wrapper_depth += 1;
            continue;
        }
        if !is_last_line && unsafe_wrapper_depth > 0 && trimmed == "}" {
            unsafe_wrapper_depth -= 1;
            continue;
        }

        out.push(line.clone());
    }

    out.join("\n")
}

fn extract_function_name(block: &str) -> Option<String> {
    let header = block.lines().next()?.trim();
    let fn_pos = header.find("fn ")?;
    let after_fn = &header[fn_pos + 3..];
    let name: String = after_fn
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn deterministic_line_diff(left: &str, right: &str) -> Vec<String> {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();
    let mut output = Vec::new();
    let max_len = left_lines.len().max(right_lines.len());

    for index in 0..max_len {
        let line_number = index + 1;
        match (left_lines.get(index), right_lines.get(index)) {
            (Some(left_line), Some(right_line)) if left_line == right_line => {}
            (Some(left_line), Some(right_line)) => {
                output.push(format!("@@ line {line_number} @@"));
                output.push(format!("- {}", left_line));
                output.push(format!("+ {}", right_line));
            }
            (Some(left_line), None) => {
                output.push(format!("@@ line {line_number} @@"));
                output.push(format!("- {}", left_line));
                output.push("+ <missing>".to_string());
            }
            (None, Some(right_line)) => {
                output.push(format!("@@ line {line_number} @@"));
                output.push("- <missing>".to_string());
                output.push(format!("+ {}", right_line));
            }
            (None, None) => {}
        }
    }

    output
}

fn import_kind_text(kind: SorobanImportKind) -> &'static str {
    match kind {
        SorobanImportKind::EnvBuiltin => "env_builtin",
        SorobanImportKind::EnvUnknown => "env_unknown",
        SorobanImportKind::NonEnv => "non_env",
    }
}

fn edge_kind_text(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Fallthrough => "fallthrough",
        EdgeKind::BranchTrue => "branch_true",
        EdgeKind::BranchFalse => "branch_false",
        EdgeKind::BackEdge => "back_edge",
        EdgeKind::Unconditional => "unconditional",
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use clap::CommandFactory;
    use serde_json::json;

    use super::{normalize_source_for_scoring, run_from, Cli, CliError};

    #[test]
    fn help_lists_required_commands() {
        let mut command = Cli::command();
        let mut buffer = Vec::new();
        command
            .write_long_help(&mut buffer)
            .expect("help output should render");

        let help = String::from_utf8(buffer).expect("help output must be UTF-8");
        assert!(help.contains("decompile"));
        assert!(help.contains("score"));
        assert!(help.contains("explain"));
        assert!(help.contains("diff"));
    }

    #[test]
    fn decompile_outputs_wat_and_rust_summaries() {
        let wasm = fixture_path("sections_imports_exports.wasm");
        let output = run_from(["sorcat-cli", "decompile", wasm.to_str().unwrap()])
            .expect("decompile should succeed for valid fixture");

        assert!(output.contains("== WAT =="));
        assert!(output.contains("== RUST SUMMARY =="));
        assert!(output.contains("(export \"adder\""));
        assert!(output.contains("pub fn adder(arg0: i32, arg1: i32) -> i32"));
    }

    #[test]
    fn scoring_normalization_keeps_public_contract_surface_without_entry_name_shortcuts() {
        let source = r#"
            #![no_std]
            use core::panic::PanicInfo;
            #[panic_handler]
            fn panic(_info: &PanicInfo) -> ! {
                loop {}
            }
            pub extern "C" fn entry_contract() -> i64 {
                helper()
            }
            pub fn helper_pub() -> i64 {
                helper()
            }
            fn helper() -> i64 {
                1
            }
        "#;

        let projected = normalize_source_for_scoring(source);
        assert!(
            !projected.contains("fn panic("),
            "panic handlers should be excluded from scoring projection"
        );
        assert!(
            projected.contains("fn helper_pub() -> i64"),
            "scoring normalization should preserve additional public functions (no entry-only shortcut)"
        );
        assert!(
            !projected.contains("fn helper() -> i64"),
            "scoring normalization should exclude non-public helpers from interface scoring scope"
        );
        assert!(
            projected.contains("fn entry_contract() -> i64"),
            "entrypoint functions should remain in scoring normalization"
        );
    }

    #[test]
    fn explain_outputs_decode_cfg_ssa_and_imports() {
        let wasm = fixture_path("sections_imports_exports.wasm");
        let output = run_from(["sorcat-cli", "explain", wasm.to_str().unwrap(), "adder"])
            .expect("explain should succeed for existing export");

        assert!(output.contains("decode.import_functions="));
        assert!(output.contains("imports:"));
        assert!(output.contains("cfg:"));
        assert!(output.contains("ssa:"));
    }

    #[test]
    fn diff_reports_no_changes_for_identical_inputs() {
        let wasm = fixture_path("ssa_sequences.wasm");
        let output = run_from([
            "sorcat-cli",
            "diff",
            wasm.to_str().unwrap(),
            wasm.to_str().unwrap(),
        ])
        .expect("diff should succeed for valid fixtures");

        assert!(output.contains("no differences detected"));
    }

    #[test]
    fn diff_reports_changes_for_different_modules() {
        let left = fixture_path("sections_imports_exports.wasm");
        let right = fixture_path("ssa_sequences.wasm");
        let output = run_from([
            "sorcat-cli",
            "diff",
            left.to_str().unwrap(),
            right.to_str().unwrap(),
            "--format",
            "rust",
        ])
        .expect("diff should succeed for valid fixtures");

        assert!(output.contains("diff format=rust"));
        assert!(output.contains("@@ line"));
    }

    #[test]
    fn score_runs_manifest_load_and_report_path() {
        let root = unique_temp_root();
        let manifest_path = write_minimal_score_fixture(&root);

        let output = run_from([
            "sorcat-cli",
            "score",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--corpus-root",
            root.to_str().unwrap(),
            "--min-mean-ast-score",
            "0.0",
            "--min-builtin-coverage",
            "0.0",
        ])
        .expect("score path should succeed on deterministic fixture");

        assert!(output.contains("contracts_scored=1"));
        assert!(output.contains("mean_ast_score="));
        assert!(output.contains("builtin_coverage="));
        assert!(output.contains("report_json="));
    }

    #[test]
    fn score_rejects_metadata_id_mismatch() {
        let root = unique_temp_root();
        let manifest_path = write_minimal_score_fixture(&root);

        let metadata_path = root.join("contracts/synthetic/sample_v1/metadata.json");
        std::fs::write(
            &metadata_path,
            "{\"id\":\"synthetic/not_sample_v1\",\"sequence\":1}\n",
        )
        .unwrap_or_else(|err| panic!("failed to overwrite metadata fixture: {err}"));

        let error = run_from([
            "sorcat-cli",
            "score",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--corpus-root",
            root.to_str().unwrap(),
            "--min-mean-ast-score",
            "0.0",
            "--min-builtin-coverage",
            "0.0",
        ])
        .expect_err("metadata id mismatch should fail");

        assert!(
            matches!(
                error,
                CliError::InvalidArgument {
                    field: "metadata_path.id",
                    ..
                }
            ),
            "expected structured metadata id error"
        );
    }

    #[test]
    fn score_default_thresholds_pass_on_committed_locked_corpus() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/corpus/manifest.v1.json");
        let corpus_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/corpus");

        let output = run_from([
            "sorcat-cli",
            "score",
            "--manifest",
            manifest.to_str().expect("manifest path should be utf-8"),
            "--corpus-root",
            corpus_root
                .to_str()
                .expect("corpus root path should be utf-8"),
        ])
            .expect("committed locked corpus should satisfy default score thresholds");

        assert!(
            output.contains("contracts_scored=80"),
            "score output should include all committed variants",
        );
        assert!(
            output.contains("mean_ast_score="),
            "score output should report mean ast score",
        );
        assert!(
            output.contains("builtin_coverage="),
            "score output should report builtin coverage",
        );
        assert!(
            output.contains("submission_ready="),
            "score output should report submission readiness status",
        );
    }

    #[test]
    fn score_submission_ready_mode_blocks_when_provenance_is_pending() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/corpus/manifest.v1.json");
        let corpus_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/corpus");

        let error = run_from([
            "sorcat-cli",
            "score",
            "--manifest",
            manifest.to_str().expect("manifest path should be utf-8"),
            "--corpus-root",
            corpus_root
                .to_str()
                .expect("corpus root path should be utf-8"),
            "--require-submission-ready",
        ])
        .expect_err("submission ready mode should block when provenance remains pending");

        assert!(
            matches!(
                error,
                CliError::InvalidArgument {
                    field: "require_submission_ready",
                    ..
                }
            ),
            "expected pending provenance to block submission-ready mode",
        );
    }

    #[test]
    fn score_rejects_lowered_thresholds_for_locked_corpus() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/corpus/manifest.v1.json");
        let corpus_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/corpus");

        let error = run_from([
            "sorcat-cli",
            "score",
            "--manifest",
            manifest.to_str().expect("manifest path should be utf-8"),
            "--corpus-root",
            corpus_root
                .to_str()
                .expect("corpus root path should be utf-8"),
            "--min-mean-ast-score",
            "0.0",
            "--min-builtin-coverage",
            "0.0",
        ])
        .expect_err("locked corpus should reject lowered score thresholds");

        assert!(
            matches!(
                error,
                CliError::InvalidArgument {
                    field: "min_mean_ast_score",
                    ..
                }
            ),
            "expected lowered threshold rejection on locked corpus",
        );
    }

    #[test]
    fn decompile_missing_file_returns_structured_error() {
        let error = run_from([
            "sorcat-cli",
            "decompile",
            "fixtures/wasm/does-not-exist.wasm",
        ])
        .expect_err("missing file should return an error, not panic");

        assert!(
            matches!(error, CliError::Io { .. }),
            "expected IO error for missing input file"
        );
    }

    #[test]
    fn decompile_respects_parse_limit_flags() {
        let wasm = fixture_path("sections_imports_exports.wasm");
        let error = run_from([
            "sorcat-cli",
            "decompile",
            wasm.to_str().unwrap(),
            "--max-wasm-bytes",
            "8",
        ])
        .expect_err("max-wasm-bytes should enforce parse limits before decompilation");

        assert!(
            matches!(error, CliError::Core { .. }),
            "expected structured core error when parse limits are exceeded",
        );
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/wasm")
            .join(name)
    }

    fn unique_temp_root() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("sorcat-cli-score-fixture-{id}"));

        if root.exists() {
            std::fs::remove_dir_all(&root)
                .unwrap_or_else(|err| panic!("failed to clean temp fixture root: {err}"));
        }
        std::fs::create_dir_all(&root)
            .unwrap_or_else(|err| panic!("failed to create temp fixture root: {err}"));
        root
    }

    fn write_minimal_score_fixture(root: &Path) -> PathBuf {
        let contract_root = root.join("contracts/synthetic/sample_v1");
        std::fs::create_dir_all(contract_root.join("src"))
            .unwrap_or_else(|err| panic!("failed to create src directory: {err}"));
        std::fs::create_dir_all(contract_root.join("wasm"))
            .unwrap_or_else(|err| panic!("failed to create wasm directory: {err}"));

        std::fs::write(
            contract_root.join("src/lib.rs"),
            "pub fn sample(a: i32, b: i32) -> i32 { a + b }\n",
        )
        .unwrap_or_else(|err| panic!("failed to write source fixture: {err}"));
        std::fs::write(
            contract_root.join("metadata.json"),
            "{\"id\":\"synthetic/sample_v1\",\"sequence\":1}\n",
        )
        .unwrap_or_else(|err| panic!("failed to write metadata fixture: {err}"));

        let wasm_fixture = fixture_path("sections_imports_exports.wasm");
        std::fs::copy(
            wasm_fixture,
            contract_root.join("wasm/debug-with-names.wasm"),
        )
        .unwrap_or_else(|err| panic!("failed to copy wasm fixture: {err}"));

        let manifest_json = json!({
            "schema_version": "1.0.0",
            "locked": false,
            "contracts": [
                {
                    "id": "synthetic/sample_v1",
                    "category": "synthetic",
                    "rust_source": "contracts/synthetic/sample_v1/src/lib.rs",
                    "metadata_path": "contracts/synthetic/sample_v1/metadata.json",
                    "variants": [
                        {
                            "profile": "debug",
                            "include_debug_names": true,
                            "sdk_version": "23.0.0",
                            "wasm_path": "contracts/synthetic/sample_v1/wasm/debug-with-names.wasm"
                        }
                    ]
                }
            ]
        });

        let manifest_path = root.join("manifest.v1.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest_json)
                .expect("fixture manifest JSON should serialize"),
        )
        .unwrap_or_else(|err| panic!("failed to write manifest fixture: {err}"));
        manifest_path
    }
}

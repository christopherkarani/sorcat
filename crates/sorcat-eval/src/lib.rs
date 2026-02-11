pub mod ast;
pub mod corpus;
pub mod report;
pub mod scoring;

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalErrorKind {
    Io,
    Json,
    InvalidManifest,
    InvalidInput,
    ThresholdNotMet,
    EmptyReport,
}

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("failed to read file at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON at `{path}`: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid corpus manifest: {message}")]
    InvalidManifest { message: String },
    #[error("invalid input for `{field}`: {message}")]
    InvalidInput {
        field: &'static str,
        message: String,
    },
    #[error("threshold `{metric}` not met: actual={actual:.6}, minimum={minimum:.6}")]
    ThresholdNotMet {
        metric: &'static str,
        actual: f64,
        minimum: f64,
    },
    #[error("deterministic evaluation report cannot be empty")]
    EmptyReport,
}

impl EvalError {
    pub fn kind(&self) -> EvalErrorKind {
        match self {
            Self::Io { .. } => EvalErrorKind::Io,
            Self::Json { .. } => EvalErrorKind::Json,
            Self::InvalidManifest { .. } => EvalErrorKind::InvalidManifest,
            Self::InvalidInput { .. } => EvalErrorKind::InvalidInput,
            Self::ThresholdNotMet { .. } => EvalErrorKind::ThresholdNotMet,
            Self::EmptyReport => EvalErrorKind::EmptyReport,
        }
    }
}

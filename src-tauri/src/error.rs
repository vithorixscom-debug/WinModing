use serde::Serialize;
use std::path::PathBuf;

/// Domain error serialized to the frontend as `{ code, message }`.
#[derive(Debug, thiserror::Error)]
pub enum LauncherError {
    #[error("network request to {url} failed: {source}")]
    Network {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("download interrupted for {url}: received {received} of {expected} bytes")]
    NetworkInterrupted {
        url: String,
        received: u64,
        expected: u64,
    },
    #[error("hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("download {url} has no declared hash")]
    MissingHash { url: String },
    #[error(
        "insufficient memory: need {needed_mb} MiB, available {available_mb} MiB after reserve"
    )]
    InsufficientMemory { needed_mb: u64, available_mb: u64 },
    #[error("Java {major} was not found")]
    JavaMissing { major: u32 },
    #[error("version document is corrupt: {reason}")]
    CorruptVersion { reason: String },
    #[error("{provider} returned HTTP {status}: {body}")]
    Provider {
        provider: String,
        status: u16,
        body: String,
    },
    #[error("{provider} rate limited the request")]
    RateLimited { provider: String },
    #[error("authentication failed: {reason}")]
    Auth { reason: String },
    #[error("instance {id} is invalid: {reason}")]
    Instance { id: String, reason: String },
    #[error("instance {id} is already running")]
    AlreadyRunning { id: String },
    #[error("missing game files: {detail}")]
    MissingAssets { detail: String },
    #[error("filesystem error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("metadata store error: {0}")]
    Cache(String),
    #[error("{0}")]
    Message(String),
}

impl LauncherError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Network { .. } => "network",
            Self::NetworkInterrupted { .. } => "interrupted",
            Self::HashMismatch { .. } => "hash_mismatch",
            Self::MissingHash { .. } => "missing_hash",
            Self::InsufficientMemory { .. } => "insufficient_memory",
            Self::JavaMissing { .. } => "java_missing",
            Self::CorruptVersion { .. } => "corrupt_version",
            Self::Provider { .. } => "provider",
            Self::RateLimited { .. } => "rate_limited",
            Self::Auth { .. } => "auth",
            Self::Instance { .. } => "instance",
            Self::AlreadyRunning { .. } => "already_running",
            Self::MissingAssets { .. } => "missing_assets",
            Self::Io { .. } => "io",
            Self::Cache(_) => "cache",
            Self::Message(_) => "internal",
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network { .. } | Self::NetworkInterrupted { .. } | Self::RateLimited { .. } => {
                true
            }
            Self::Message(text) => text.contains("network request"),
            _ => false,
        }
    }

    pub fn should_try_mirror(&self) -> bool {
        match self {
            Self::Network { .. } | Self::NetworkInterrupted { .. } | Self::RateLimited { .. } => {
                true
            }
            Self::Message(text) => text.contains("network request"),
            Self::Provider { status, .. } => {
                // Include 404/410 so CDN host swaps (cdn ↔ cdn-alt, forgecdn mirrors) can run.
                matches!(
                    *status,
                    0 | 401 | 403 | 404 | 408 | 410 | 429 | 451 | 500 | 502 | 503 | 504 | 522
                        | 523 | 530
                )
            }
            _ => false,
        }
    }
}

impl Serialize for LauncherError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("LauncherError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

pub fn io_at(path: impl Into<PathBuf>, source: std::io::Error) -> LauncherError {
    LauncherError::Io {
        path: path.into().display().to_string(),
        source,
    }
}

impl From<rusqlite::Error> for LauncherError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Cache(value.to_string())
    }
}

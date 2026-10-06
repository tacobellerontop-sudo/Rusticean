use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not reach any Roblox download mirror (last error: {0})")]
    NoMirror(String),

    #[error("channel '{0}' does not exist or is not accessible")]
    InvalidChannel(String),

    #[error("unexpected package manifest: {0}")]
    BadManifest(String),

    #[error("checksum mismatch for {package}: expected {expected}, got {actual}")]
    Checksum {
        package: String,
        expected: String,
        actual: String,
    },

    #[error("failed to extract {package}: {message}")]
    Extract { package: String, message: String },

    #[error("operation cancelled")]
    Cancelled,
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

/// Errors raised by the sandbox. Every message is written to be relayed to the agent or
/// shown in the panel verbatim, as the TS `Error` messages were.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A user/agent-facing failure (bad compose YAML, unknown service, Docker missing, …).
    #[error("{0}")]
    Message(String),
    /// The Engine API could not be reached. Mirrors TS `DockerEngineUnavailable`.
    #[error("{0}")]
    EngineUnavailable(String),
    /// Tool input failed validation (`Invalid input: …`).
    #[error("{0}")]
    InvalidInput(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub(crate) fn msg(message: impl Into<String>) -> Self {
        Error::Message(message.into())
    }

    /// True for the TS `DockerEngineUnavailable` class.
    pub fn is_engine_unavailable(&self) -> bool {
        matches!(self, Error::EngineUnavailable(_))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

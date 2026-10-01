//! Crate error type.

/// Agent run and delegation failures. `Display` is the message the TS code threw.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// `new Error(`Agent not found: ${agentId}`)`.
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    /// The first Converse call of a run exceeded `timeoutMs` (`new Error('Chat timeout')`).
    #[error("Chat timeout")]
    ChatTimeout,
    /// A Converse call failed; the message is the backend's.
    #[error("{0}")]
    Converse(String),
    /// Persisting a session message failed.
    #[error("{0}")]
    Session(String),
    /// `SubAgentPolicyError`: a delegation that was refused or timed out. Reported to the calling
    /// model verbatim.
    #[error("{0}")]
    Policy(String),
    /// The run task itself failed (panicked or was aborted).
    #[error("{0}")]
    Run(String),
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, Error>;

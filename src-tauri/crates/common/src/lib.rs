//! Shared logic for the Bedrock Engineer backend: agent types and schema validation, agent config
//! files, delegation policy, system-prompt helpers, tool naming, logging, and the login-shell PATH.
//!
//! Port of `src/common/{agents,utils,validation,logger}`, `src/types/agent-chat*.ts`, and the
//! filesystem logic of `src/main/handlers/{agent,help}-handlers.ts`.

pub mod agent;
pub mod agent_files;
pub mod delegation;
pub mod help;
pub mod json_file;
pub mod logger;
pub mod placeholders;
pub mod shell_env;
pub mod tool_description;
pub mod tool_names;
pub mod tool_rules;
pub mod validation;
pub mod zod;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("logger error: {0}")]
    Logger(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod schema_tests;

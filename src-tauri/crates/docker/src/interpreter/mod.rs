//! Code Interpreter: one-shot Python containers. Port of the execution half of
//! `src/preload/tools/handlers/interpreter/**` (`DockerExecutor`, `SecurityManager`,
//! `FileManager`, `TaskManager`, and `CodeInterpreterTool`'s operations).
//!
//! The tools crate wraps [`CodeInterpreter`] as its `codeInterpreter` Tool: it can call
//! [`CodeInterpreter::validate_input`] and [`CodeInterpreter::execute`] with the raw tool
//! input, and use [`tool::tool_spec`] for the Bedrock tool specification.

pub mod executor;
pub mod files;
pub mod logger;
pub mod security;
pub mod tasks;
pub mod tool;
pub mod types;

pub use executor::DockerExecutor;
pub use files::FileManager;
pub use logger::{LogFacadeLogger, RecordingLogger, ToolLogger};
pub use security::SecurityManager;
pub use tasks::TaskManager;
pub use tool::{
    CodeInterpreter, CodeInterpreterOutput, ValidationResult, TOOL_DESCRIPTION, TOOL_NAME,
};
pub use types::*;

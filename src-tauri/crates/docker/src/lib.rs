//! Docker sandbox: per-chat containers, compose stacks, terminal, activity log, and the
//! Code Interpreter's one-shot Python containers.
//!
//! Port of `src/main/api/docker/**` and the execution half of
//! `src/preload/tools/handlers/interpreter/**`. Everything shells out to the `docker` CLI
//! with the same argv, labels and names the Electron build uses, so containers it created
//! are recognized here; the interactive terminal and the status/insight polls talk to the
//! Engine API over the local socket exactly as the TS does.
//!
//! State that was module-global in TS (activity log, tracked execs, terminals, caches)
//! lives on [`SandboxManager`] and its parts, so tests build isolated instances.
//!
//! Test mapping (TS → Rust):
//! - `naming.test.ts` → [`naming`] tests
//! - `composeWriter.test.ts` → [`compose_writer`] tests
//! - `dockerEngine.test.ts` → [`engine`] tests
//! - `sandboxActivity.test.ts` → [`activity`] tests
//! - `sandboxTerminal.test.ts` → [`terminal`] tests
//! - `sandbox.integration.test.ts` → `tests/sandbox_integration.rs` (`#[ignore]`)
//! - `src/test/sandbox/sts.client.test.ts` → `tests/sts_client.rs` (`#[ignore]`)
//! - `DockerExecutor.test.ts`, `CodeInterpreterTool.test.ts` → [`interpreter`] tests
//! - `DockerExecutor.integration.test.ts`, `CodeInterpreterTool.integration.test.ts` →
//!   `tests/interpreter_integration.rs` (`#[ignore]`)

pub mod activity;
pub mod availability;
pub mod compose_writer;
pub mod engine;
mod error;
pub mod events;
pub mod exec;
pub mod interpreter;
pub mod manager;
pub mod naming;
pub mod output_patterns;
pub mod runner;
pub mod terminal;
pub mod types;
mod util;

pub use error::{Error, Result};
pub use events::{EventSink, NoopSink, RecordingSink, SandboxEvent};
pub use manager::{SandboxComposeFile, SandboxInsights, SandboxManager, Settings};
pub use runner::{BoxFuture, CommandRunner, RunOptions, RunResult, TokioRunner};
pub use types::*;

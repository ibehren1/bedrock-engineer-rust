# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Bedrock Engineer (the "Bedrock Engineer Rust Fork") is a Tauri v2 desktop app (Mac/Windows/Linux) with a Rust backend that provides autonomous AI agent capabilities powered by Amazon Bedrock. It supports chat with tool use, website generation, diagram generation, Step Functions generation, background agents, an agent directory, MCP integration, a per-chat Docker sandbox, local knowledge bases for agents, a shared memory list, a Files panel over the project folder, a local terminal and in-app updates. It started as an Electron app (upstream aws-samples/bedrock-engineer); the Electron main and preload processes were ported to Rust (history: `docs/port/PLAN.md`) and removed. Voice chat (Nova Sonic) was dropped in the port.

## Build & Dev Commands

```bash
npm ci                    # Install dependencies (includes the Tauri CLI)
npm run dev               # Run in development mode (tauri dev: Vite dev server + app window)
npm run build             # Typecheck, then tauri build for this machine's architecture
npm run build:mac         # Build for macOS (universal .app + .dmg; needs both Rust mac targets)
npm run build:win         # Build for Windows (NSIS -setup.exe)
npm run build:linux       # Build for Linux (AppImage + deb)
npm run dev:renderer      # Vite dev server only (http://localhost:5173)
npm run build:renderer    # Vite build only -> dist/renderer (what Tauri bundles)

npm run typecheck         # Run both node (vite.config.ts) and web (renderer) typechecks
npm run lint              # ESLint
npm run lint:fix          # ESLint with auto-fix
npm run format            # Prettier

npm test                  # Renderer unit tests (Jest) + release-script tests (node --test)
npm run test:watch        # Unit tests in watch mode
npx jest path/to/file.test.ts              # Run a single test file

# Rust backend (from src-tauri/)
cargo test --workspace                     # All Rust tests (integration tests are #[ignore])
cargo test --workspace -- --ignored        # Integration tests (need AWS credentials / Docker)
cargo clippy --workspace --all-targets
cargo build -p app                         # Debug build of the app binary

make test                 # cargo test --workspace + npm test (what CI runs)
make notices              # Regenerate notices/rust.md + notices/frontend.md (needs cargo-about: cargo install cargo-about --locked --features cli)
```

The universal macOS build needs `rustup target add aarch64-apple-darwin x86_64-apple-darwin` (`make build-mac` does it). The Rust build compiles C code (bundled SQLite via rusqlite, sqlite-vec, oniguruma for the tokenizer), so a C toolchain is needed on every platform (Xcode CLT / MSVC Build Tools / gcc); Linux also needs the apt packages listed in `.github/workflows/build-and-release.yml`. Bundles land in `src-tauri/target/[<triple>/]release/bundle/`.

## Architecture

A **React + TypeScript renderer** (built by Vite, `vite.config.ts`) runs in a **Tauri v2** webview; the backend is a **Rust Cargo workspace** under `src-tauri/`.

### Rust backend (`src-tauri/`)

- `app/` — the Tauri binary: `main.rs` (window setup, custom URI schemes — `htmlpreview://`, `workspacefile://`, `chathistory://`, `chatexport://` for the PDF export — lifecycle), `commands/` (one module per `window.*` namespace; every command is registered in `commands/mod.rs`), `command_list.rs` (reads the registered command list for `build.rs` and the ACL; `MAIN_WINDOW_ONLY_PREFIXES`), `backend.rs` (shared services), `state.rs` / `store_sync.rs` (settings store and change events), `security.rs` (webview origin checks), `menu.rs` (app menu, About, context menu), `tauri.conf.json` (bundle config, CSP, resources, updater), `capabilities/` (`main-window`, `file-viewer`, `camera-preview`, `default`) + `permissions/`
- `crates/store` — the settings file (`config.json`, electron-store–compatible format and location), defaults and migrations
- `crates/models` — model catalog, pricing, prompt-cache metadata (`data/models.json`, generated from `src/common/models/models.ts` by `scripts/port/gen-models-json.mjs`)
- `crates/bedrock` — AWS client factory (static credentials or named profiles, proxy), Converse / ConverseStream, image, video, agents, flows, guardrails, translate, inference profiles, structured output
- `crates/tools` — `Tool` trait + `ToolRegistry`, the built-in tools (filesystem, command, web, Bedrock, system/screen/camera, Docker sandbox, code interpreter, think, todo, memory), the MCP tool adapter
- `crates/agents` — agent lookup, system prompts, the Rust-side agent loop and sub-agent delegation
- `crates/background` — background agents: sessions, cron scheduler, execution history, notifications, pub/sub
- `crates/docker` — per-chat Docker sandbox (compose, terminal, activity log) and the Code Interpreter containers
- `crates/mcp` — MCP clients (stdio, SSE, streamable HTTP), tool adapter, registry search
- `crates/knowledge` — agents' local knowledgebases: SQLite (sqlite-vec + FTS5) per knowledgebase, document extraction, website crawler, Bedrock / on-device (candle, multilingual-e5-small) embeddings, background ingestion jobs, and the built-in `local-knowledgebase` MCP server
- `crates/attachments` — per-chat file attachments
- `crates/history` — chat history (`chat-sessions/`, `chat-sessions-meta.json`)
- `crates/strands` — Strands Agents project export
- `crates/common` — agent types and schema validation, agent config files, delegation policy, prompt helpers, logging
- `crates/workspace_files` — the Files panel: list, read and write files under the project folder (`projectPath`, never outside it), the `workspacefile://` byte server, the debounced project watcher
- `crates/local_terminal` — interactive shells on the user's machine (real PTY) for the chat's Terminal tab; no tool reaches them
- `crates/usage` — the usage ledger behind the Accounting page: every billable Bedrock call (reported through `bedrock::usage`'s process-wide sink) as a priced SQLite row in `<userData>/usage/usage.db`, with day / month / model / feature summaries

### Renderer (`src/renderer/src/`)

- React 18 + React Router (hash router) + Tailwind CSS + Flowbite React
- `main.tsx` awaits `installTauriBridge()` (`lib/tauriBridge.ts`) before importing the app: it installs `window.api`, `window.store`, `window.file`, `window.chatHistory`, `window.memory`, `window.workspaceFiles`, `window.appWindow`, `window.ipc`, `window.logger` backed by Tauri `invoke()` / events and hydrates the synchronous caches (store, chat history, tool specs)
- `lib/api.ts` — chat streaming (Tauri `Channel`), converse, structured output, model and tag lists
- `contexts/` — `SettingsContext` (AWS creds, model, agents, tools), `ChatHistoryContext`, `AgentDirectoryContext`, `WebsiteGeneratorContext`, `MemoryContext`, `FilesPanelContext`, `AppUpdateContext`
- `pages/` — ChatPage, MyAgentsPage, MemoryPage, AccountingPage, WebsiteGeneratorPage, DiagramGeneratorPage, StepFunctionsGeneratorPage, AgentDirectoryPage, BackgroundAgentPage, SettingPage (tabs in `pages/SettingPage/settingTabs.ts`: General, AWS, Models, Chat, Workspace, Sandbox, Knowledge Base, Updates), FileViewerPage (pop-out file windows)
- `i18n/` — English (`en.ts`) and Japanese (`ja.ts`) translations
- Path aliases: `@renderer` → `src/renderer/src`, `@` → `src/`, `@common` → `src/common`

### Common (`src/common/`) — renderer-side shared logic

- `models/` — model definitions, pricing, prompt cache config (source of truth for the Rust `models` crate's JSON)
- `agents/` — delegation policy, system prompt / tool rule builder
- `mcp/` — MCP schemas, registry search helpers
- `utils/` — placeholder replacement

### Types (`src/types/`)

- `window.ts` + `bridge/` — the `window.*` API surface the bridge installs (`API`, `ConfigStore`/`StoreScheme`, `FileApi`, `ChatHistoryApi`, memory, workspace files, logger, ipc)
- `knowledge.ts` — local knowledge base settings and `window.api.knowledge` shapes; `sandbox.ts` — Docker sandbox types
- `agent-chat.ts` / `agent-chat.schema.ts` — core types for agents, tools, MCP config (Zod schemas)
- `tools.ts` — built-in tool name union type and tool-related types
- `llm.ts` — model IDs, regions, inference parameters, thinking mode
- `ipc.ts` — channel → params/result types for `window.ipc.invoke`

## Key Patterns

- **Renderer ↔ Rust bridge**: the renderer calls `window.api.*` / `window.store.*` etc.; `tauriBridge.ts` maps each method to a `#[tauri::command]` (snake_case name, one named-object argument, `Result<T, String>`). The full contract and command table are in `docs/port/BRIDGE.md`; follow its rules when adding a method.
- **Events**: Rust → renderer pushes are Tauri events (`store-changed`, `memory-changed`, `workspace-files-changed`, `knowledge:progress` / `knowledge:changed` / `knowledge:model-download`, `background-agent:*`, `camera:capture-request`, `open-settings`, `pubsub:*`); chat streaming uses a Tauri `Channel` per request.
- **Main-window-only commands**: commands whose names start with a prefix in `MAIN_WINDOW_ONLY_PREFIXES` (`app/src/command_list.rs`: `local_terminal_`, `app_update_`, `knowledge_`, `usage_`) are granted only to the main window's capability; extend that list for new main-window-only namespaces.
- **Tool system**: each tool implements the Rust `Tool` trait and is registered in `ToolRegistry` by name; the renderer runs the chat tool-use loop and executes tools through `tools_execute`. Background agents and sub-agents run their loops in Rust (`crates/agents`).
- **Agent configuration**: Agents are YAML files (`.bedrock-engineer/agents/`) with system prompts, tool selections, and scenarios. Custom agents are stored in the settings file.
- **AWS credentials**: Supports both access key/secret and named AWS profiles. Region and profile are stored in the settings file at key `aws`.
- **Help**: the sidebar's Help button opens `pages/HelpPage` (the user guide, offline, searchable); its "Ask a question" starts the Help chat, which gets the guide sections matching each question through `useAgentChat`'s `prepareTurn`. The search runs over `docs/help-kb.db`, the guide's knowledge base that ships with the app (`crates/knowledge/src/help.rs`, built with the on-device model by `make help-kb`). **Whenever `docs/USER_GUIDE.md` changes, `docs/help-kb.db` must be rebuilt**: the pre-commit hook (`scripts/hooks/pre-commit`) does it when the guide is staged, and `cargo test` fails on a stale index.
- **Usage budget**: the store key `usageBudget` (`{ monthlyUsd, alertAt }`) drives the Accounting page's budget card, the sidebar badge and the OS notifications (`app/src/budget.rs`).
- **Settings file**: `config.json` in the OS app-data dir under the product name (`Bedrock Engineer`). On first launch the app moves the config dir from the fork's earlier name (`LEGACY_APP_NAME` in `app/src/main.rs`), so settings and chat history carry over. The bundle identifier is `com.bedrock-engineer`.
- **MCP integration**: MCP servers are configured per-agent with support for both command (stdio) and URL (SSE/streamable HTTP) connection types. Agents with local knowledge bases also get the in-process `local-knowledgebase` server (`connectionType: 'builtin'`, never stored on the agent).
- **Usage metering**: new code that calls a billable Bedrock API should report a `bedrock::usage::UsageEvent` (every service in `crates/bedrock` already does: tokens, or units such as images, video seconds, characters and guardrail text units); renderer Bedrock calls pass a `usageContext` (`src/types/usage.ts`) so the Accounting page can break cost down by feature. Prices come from `src/common/models/models.ts` (`pricing`, `EMBEDDING_MODEL_PRICING`, `UNIT_PRICING`, and `RETIRED_MODEL_PRICING` for models removed from the registry: when you retire a model, move its price there so recorded usage stays priced). A model's `pricingRules` record profile-prefix multipliers and long-context tiers; add them when a model's pricing note mentions either.
- **Local knowledge bases**: per agent, managed by `crates/knowledge` through the `knowledge_*` commands (`window.api.knowledge`); their settings are the store key `knowledgebase` (not to be confused with `knowledgeBases`, the Bedrock Knowledge Base list).
- **Security**: the app CSP is in `tauri.conf.json`; model-written HTML previews render in the opaque-origin `htmlpreview://` scheme; `app/src/security.rs` decides which webview origins count as the app, and IPC is limited by `app/capabilities/`.

## Changelog (CHANGELOG.md)

`CHANGELOG.md` is the fork's dated update summary, and its **topmost dated section is published
verbatim as the GitHub release notes** for each build (see the "Build release notes from CHANGELOG"
step in `.github/workflows/build-and-release.yml`). Treat it as release-facing copy, not an internal
log.

Rules:

- **Every commit that changes user-visible behavior updates `CHANGELOG.md` in the same commit.**
  That covers features, UI changes, model additions, pricing, bug fixes, and anything that alters
  what a user sees or how they use the app. Include the changelog edit in the commit itself, not a
  follow-up.
- **Skip it for changes with no user-visible effect**: refactors, test-only changes, comment or
  formatting passes, and internal tooling that doesn't reach a build. When in doubt, add an entry.
- **Write for the person installing the build**, in plain prose: what changed and, where it isn't
  obvious, why. Avoid file paths, symbol names, and commit-type prefixes (`feat:`, `fix:`).
- **Add to the section for today's date** (`### YYYY-MM-DD`) at the top of the file, creating it if
  it doesn't exist yet. Newest section first; never reorder or rewrite older sections.
- **One bullet per change**, appended to the end of today's section so entries stay in the order
  they happened.
- **Don't reference the changelog update itself** in an entry — describe the actual change.
- `README.md` describes the fork feature-by-feature; when a change makes part of it wrong (or adds
  something worth showing), update the README in the same commit too.

## Testing

- Rust: `#[cfg(test)]` modules and `tests/` in each crate; run `cargo test --workspace` from `src-tauri/`. Integration tests that need real AWS credentials or Docker are `#[ignore]` (run with `-- --ignored`).
- Renderer: Jest `*.test.ts` under `src/renderer` and `src/common` (`npm test`, config `jest.config.js`, TypeScript via `tsconfig.test.json`).
- Coverage history: `docs/port/COVERAGE_PARITY.md`.

## Development Agent

The project includes a pre-configured development agent at `.bedrock-engineer/agents/developer-for-bedrock-engineer.yaml` recommended for use during development.

# Rust/Tauri Port Plan

Re-platform the Electron main + preload layers onto a Tauri v2 Rust backend under `src-tauri/`,
reusing the React renderer in `src/renderer/`. Nova Sonic voice chat is dropped.

**Status: complete.** Task 13 deleted the Electron main + preload code (`src/main`, `src/preload`),
the Electron build config and dependencies. Paths such as `src/main/...` and `src/preload/...`
below, in `BRIDGE.md` and in Rust doc comments name that TypeScript, which is in git history up to
`c9a9a13` on `rust-port`.

Coverage checklist: `COVERAGE_PARITY.md`.

## Architecture notes

- The renderer streamed chat with `fetch(${API_ENDPOINT}/converse/stream)` in
  `src/renderer/src/lib/api.ts` (now `tauriConverseStream`), and the tool-use loop runs in the renderer
  (`pages/ChatPage/hooks/useAgentChat.ts`), calling tools through preload. The Rust side therefore
  needs a streaming converse command (Tauri v2 `Channel`) and a tool-execution command, not a full
  agent loop. Background agents and sub-agents are the loops that run in Rust.
- A renderer shim (`src/renderer/src/lib/tauriBridge.ts`) installs `window.api`, `window.store`,
  `window.file`, `window.chatHistory`, etc. backed by `invoke`, so components stay unchanged.
  `window.store.get` is synchronous today, so the shim hydrates a cache from `store_all` before
  React mounts and writes through with `store_set`.
- Bridge surface at the start of the port: ~107 distinct `window.*.*` call sites. Main + preload
  is ~37k lines of TS.

## Tasks

| # | Task | Status |
| - | ---- | ------ |
| 1 | Cargo workspace + Tauri app skeleton; `store` crate reproducing `src/preload/store.ts` (same file, same defaults and migrations); NOTICE and license metadata | done |
| 2 | Renderer on Tauri: Vite config, `@tauri-apps/api`, `tauriBridge.ts` shim with sync store cache, `frontendDist` at Vite output; remove SpeakPage / Nova Sonic UI; retained renderer tests stay on Jest | done |
| 3 | `models` crate: model registry, pricing, context limits, prompt cache metadata | done |
| 4 | `bedrock` crate: client (static creds + named profiles), Converse + ConverseStream over `Channel`; replace `lib/api.ts` fetches | done |
| 5 | Chat history and file commands (`window.chatHistory`, `window.file`, dialogs) | done |
| 6 | `tools` crate core: filesystem tools (incl. gitignore matcher, Excel→CSV), ExecuteCommand with allowlist + output patterns, web (Tavily, fetch), Think, Todo | done |
| 7 | Agents: custom/shared agent YAML, validation, delegation, InvokeAgent / sub-agent runner | done |
| 8 | `mcp` crate: stdio + SSE/streamable HTTP clients, tool adapter, registry | done |
| 9 | `attachments` crate + remaining Bedrock tools (image generate/recognize, KB retrieve, Bedrock agents, flows, guardrails, translate) | done |
| 10 | Deferred: video generation, code interpreter + Docker sandbox (compose, PTY terminal, activity) | done |
| 11 | Deferred: screen/camera capture, background agents + scheduler, agent directory | done |
| 12 | Packaging: Tauri bundles for mac/win/linux, release workflow, third-party notices (`cargo about` + frontend deps) | done |
| 13 | Finalize: delete Electron main/preload and Electron deps, real bundle identifier (`com.bedrock-engineer`), README attribution/provenance section, all parity rows `done` | done |

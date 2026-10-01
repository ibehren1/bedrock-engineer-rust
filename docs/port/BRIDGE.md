# Renderer ↔ Rust bridge

The React renderer calls `window.api`, `window.store`, `window.file`, `window.chatHistory`,
`window.appWindow`, `window.ipc` and `window.logger` exactly as it did under Electron. Those
objects are installed by `src/renderer/src/lib/tauriBridge.ts`, backed by `invoke()` and Tauri
events, and typed in `src/types/window.ts` + `src/types/bridge/`. The Electron preload
(`src/preload/*.ts`, removed in Task 13; see git history) defined each method's behavior and
return shape, which the commands below reproduce.

To port a method: add a `#[tauri::command]` with the name and parameters listed below, register it
in `src-tauri/app/src/main.rs`, and return the same JSON the Electron handler returned. Nothing in
the renderer needs to change.

## Rules

1. **Command names.** Methods that were `ipcRenderer.invoke(channel, …)` use the Electron channel
   in snake_case (`:` and `-` → `_`, camelCase split): `background-agent:chat` →
   `background_agent_chat`, `window:openTaskHistory` → `window_open_task_history`. Methods the
   preload implemented itself use `<namespace>_<method>`: `bedrock_execute_tool`,
   `chat_history_create_session`, `file_read_shared_agents`, `tools_get_tool_specs`.
   `window.ipc.invoke(channel, …)` applies the channel rule at runtime (`channelToCommand`); one
   plain-object argument is passed through as the args object, anything else as `{ args: [...] }`.
2. **Arguments** are always one named object. Its keys are camelCase; Tauri maps them to
   snake_case Rust parameters (`sessionId` → `session_id`). "params object as-is" means the
   Electron handler received one object and its keys are the parameters.
3. **Errors.** Commands return `Result<T, String>`. The string may be a plain message or a JSON
   object `{"name": "...", "message": "...", ...}`; the shim turns either into an `Error` with that
   `name`/`message` (extra fields are copied onto it). Methods whose preload version never rejects
   (`window.file.*`) keep returning `{ success: false, error }` / `{ agents: [], error }` instead.
   The MCP methods unwrap the `{ success, error, … }` envelope exactly as the preload did, so the
   Rust side should return that envelope.
4. **Not yet ported.** An unregistered command rejects with
   `Error("not yet ported: <method> (<command>)")` (`NotPortedError`) and is logged once to the
   console. Nothing throws synchronously, so the UI still loads.
5. **Events.** Main → renderer pushes keep their Electron channel as the Tauri event name:
   `background-agent:task-notification`, `background-agent:task-execution-start`,
   `background-agent:task-skipped`, `context-menu-command`. Pub/sub messages for channel `c` are
   emitted as the event `pubsubEventName(c)` = `pubsub:` + `c` with every character outside
   `[A-Za-z0-9-/:_]` replaced by `_`; the payload is the published `data`.
6. **Binary data** can't go through JSON as `Uint8Array`, so it is sent as `number[]`
   (serde `Vec<u8>`): `chatAttachments.add` files, the sandbox terminal's live `data` frames
   (`bytes`, which `new Uint8Array(bytes)` accepts as is) and its backlogs (the shim converts
   `terminal.attach` / `terminal.backlog` results to `Uint8Array`, because the renderer checks
   `backlog.byteLength`).

## Startup and synchronous methods

`main.tsx` awaits `installTauriBridge()` before importing the app. It hydrates:

| Cache | Command | Used by |
| ----- | ------- | ------- |
| `window.store.get` (sync) | `store_all` → whole config object | everything |
| `window.chatHistory.getSession` / `getAllSessionMetadata` / `getRecentSessions` / `getActiveSessionId` (sync) | `chat_history_snapshot` → `{ activeSessionId: string \| null, metadata: SessionMetadata[], recent: SessionMetadata[], sessions: Record<id, ChatSession> }` | `ChatHistoryContext`, `ChatPage` |
| `window.api.tools.getToolSpecs` (sync) | `tools_get_tool_specs` → `Tool[]` (same as `ToolMetadataCollector.getToolSpecs()`) | `SettingsContext`, tool settings modal |

`metadata` and `recent` must be what `ChatSessionManager.getAllSessionMetadata()` and
`getRecentSessions()` return (sessions with at least one message whose file exists, metadata
sorted by `updatedAt` descending). `sessions` holds full sessions keyed by id: the active session
plus the recent ones (a real history can be hundreds of MB, so not all of them).

`getSession(id)` for an id not in the cache reads it **synchronously** with a sync
`XMLHttpRequest` to the `chathistory` URI scheme registered in `src-tauri/app`
(`convertFileSrc(id, 'chathistory')`, i.e. `chathistory://localhost/<id>`, or
`http://chathistory.localhost/<id>` on Windows). It answers `200` with the session JSON or `404`
with `null`, accepts only file-name-safe ids (`[A-Za-z0-9_.-]`, no leading `.`), and the result is
cached. Only app pages may read it: the request's `Origin` must be `tauri://localhost`,
`http(s)://tauri.localhost` or (debug builds) `build.devUrl`, and is echoed in
`Access-Control-Allow-Origin`; anything else gets `403` without CORS headers (see Security). Implementation: `src-tauri/crates/history` (`ChatSessionManager`, same on-disk format as
Electron) and `src-tauri/app/src/commands/chat_history.rs`.

Chat-history details that differ from a literal call-through:

- `createSession` also sends `title: "Chat " + new Date().toLocaleString()` so the default title
  stays in the renderer's locale, as the preload built it; Rust falls back to the en-US format.
- `setActiveSession(undefined)` sends no `sessionId`; Rust clears `activeSessionId` (electron-store
  threw "Use `delete()` to clear values" there).
- A `chat-sessions-meta.json` that doesn't parse is renamed to
  `chat-sessions-meta.json.corrupt-<unix-ms>` and treated as empty (the metadata is then rebuilt
  from the session files) instead of failing startup like electron-store did. One that can't be
  read at all is never written over.

Writes: `store.set(key, value)` updates the cache immediately and writes through with
`store_set { key, value }` (`store_delete { key }` when `value` is `undefined`), serialized in call
order. `store.get` returns a deep copy, like electron-store reading from disk.

Store safety (the bridge side of the Rust store's re-read-before-write):

- If `store_all` fails (or returns no object), `installTauriBridge()` rejects with
  `StoreHydrationError`; `main.tsx` shows a minimal error screen with a Retry (reload) instead of
  the app, and `store.set` is a logged no-op until a hydration succeeds, so defaults are never
  written over settings that merely failed to load.
- Rust emits `store-changed { key, value, deleted }` (top-level key) to every window after each
  successful config store `set` / `delete`, whoever made it (`store_set`, `open_directory`, the
  background scheduler, ...). The bridge updates its cache, except for keys with its own write
  still queued. Changes arriving before hydration are replayed on top of the snapshot.
- `tools_execute`, `converse`, `converse_stream`, `bedrock_*` and `background_agent_*` wait for the
  window's queued store writes (`storeFlush()`) before invoking, since Rust reads settings from the
  store.
- Closing the `main` / `task-history` window, or quitting the app, first emits
  `store-flush-request { token }` to the window; the bridge drains its write queue and answers
  `store_flush_done { token }`. The window is destroyed (or the app exits) then, or after 3 s.

After each async chat-history mutation (and in the background after the sync ones:
`deleteSession`, `deleteSessions`, `deleteAllSessions`, `setActiveSession`), the shim re-reads:
`chat_history_get_all_session_metadata`, `chat_history_get_recent_sessions`,
`chat_history_get_active_session_id` and, for the mutated session, `chat_history_get_session
{ sessionId }` (→ `ChatSession | null`).

Implemented in the renderer, no command needed: `api.bedrock.getImageGenerationModelsForRegion`
(shared model table), `api.bedrock.convertInferenceProfileToLLM` (copy of
`InferenceProfileService.convertProfileToLLM`), `appWindow.isFocused` / `api.window.isFocused`
(`getCurrentWindow().isFocused()`, falling back to `document.hasFocus()`).
`api.codeInterpreter.getCurrentWorkspacePath` is synchronous with no
backend yet and returns `null`.

Logger: `window.logger.log.*` / `createCategoryLogger(category).*` send
`logger_log { entry: { level, message, timestamp, process: "renderer", category?, ...meta } }`,
fire-and-forget (Electron: `ipcRenderer.send('logger:log', entry)`). `logger_log` writes through
`common::logger` (the winston-format daily files under `<userData>/logs`, plus stdout), with
`process: renderer`; extra fields are logged as a `meta` JSON string.

## Command table

Commands are registered in `src-tauri/app/src/commands/mod.rs` (one module per namespace). Every
row below is registered.

Also registered, beyond the table: `tools_execute` (same as `bedrock_execute_tool`),
`background_agent_task_notification` (`{ params }`, re-broadcast to every window), and the
tool-only `bedrock:*` handlers for `window.ipc.invoke`: `bedrock_generate_image`,
`bedrock_recognize_image`, `bedrock_retrieve`, `bedrock_invoke_agent`, `bedrock_invoke_flow`,
and the Nova Reel handlers `bedrock_generate_video`, `bedrock_start_video_generation`,
`bedrock_check_video_status`, `bedrock_download_video` (`src-tauri/app/src/commands/video.rs`;
each reads the handler's params object as-is; `submitTime` / `endTime` are ISO strings, not
`Date`s).

Also registered for `window.ipc.invoke` parity: `screen_capture { options? }` and
`screen_check_permissions` (the `screen:capture` / `screen:check-permissions` channels the
Electron tool called), and `camera_capture_response` (internal, see below).

### Screen, camera and task history

Commands: `src-tauri/app/src/commands/{screen,camera,window}.rs`; logic and tools:
`src-tauri/crates/tools/src/system/`.

- **Screen** (`screen_*`, screenCapture tool): `xcap` behind `tools::ScreenSource`. Same
  options (`format` png|jpeg, `quality` default 80, `outputPath`, `windowTarget`), same file
  location (`<tmpdir>/screenshot_<ms>.<format>`), same result / permission / `WindowInfo` shapes.
  The capture is scaled to fit 1920x1080 and thumbnails to 300x200, as `desktopCapturer`'s
  `thumbnailSize` did. Source ids keep Electron's `screen:<id>:0` / `window:<id>:0` form;
  `windowTarget` matches the window title or its app name (case-insensitive). macOS permission
  is `CGPreflightScreenCaptureAccess` (the first denied check also calls
  `CGRequestScreenCaptureAccess` so macOS lists the app under Screen Recording); the messages
  are the TS ones. Dock / Notification Center / widget windows are not listed.
- **Camera frames**: the tool runs in Rust, the camera only in a webview. Rust emits
  `camera:capture-request { requestId, deviceId?, quality, format }` to the `main` window; the
  bridge (`lib/cameraCapture.ts`, the TS `CameraStreamManager`) grabs a frame with
  `getUserMedia` and answers `camera_capture_response { requestId, frame: { base64Data, width,
  height, deviceId, deviceName } }` or `{ requestId, error }`. Rust saves it with the
  `camera:save-captured-image` logic (`<tmpdir>/camera_capture_<ms>.<format>`). Timeout 60 s.
  Background-agent runs need the main window open for this.
- **Camera previews**: frameless always-on-top windows labelled `camera-preview-<n>` loading
  `camera-preview.html?deviceId=&deviceName=` (built by `vite.tauri.config.ts`; capability
  `capabilities/camera-preview.json` allows dragging plus `camera_close_preview_window` and
  `camera_hide_preview_window`, the only commands the page invokes). Sizes, cascade / grid positions and the
  `{ success, message }` / status results follow `camera-handlers.ts`; positions use the
  primary monitor's logical work-area size. Opacity: `NSWindow.alphaValue` on macOS, a
  transparent window plus CSS opacity elsewhere.
- **Permissions**: `on_permission_request` allows camera access only to the `main` and
  `camera-preview-*` webviews, and only while they show an app page (`src-tauri/app/src/security.rs`);
  everything else gets the platform default. macOS still shows its TCC prompt, which needs
  `NSCameraUsageDescription` (`src-tauri/app/Info.plist`, merged by the bundler and embedded in
  dev builds).
- **`window_open_task_history { taskId }`** → `{ success, windowId, reused }` (`windowId` is
  the label `task-history`, not Electron's numeric id) or `{ success: false, error }`. Opens
  `index.html#/background-agent/task-history/<taskId>` in a 1400x900 window, or navigates the
  open one. Background-task notifications call the same function on click
  (`src-tauri/app/src/notify.rs`: `mac-notification-sys` with `wait_for_click` on macOS,
  `notify-rust` D-Bus actions on Linux; Windows has no click action).

Commands marked "(params object as-is)" take the params object's keys as their arguments
(`background_agent_chat { sessionId, config, userMessage, options }`, `bedrock_translate_text
{ text, sourceLanguage?, targetLanguage, cacheKey? }`, `todo_init { sessionId, items }`, …);
`sub_agent_invoke` reads the whole object as `SubAgentInvokeParams`.

### Backend notes (`src-tauri/app/src/backend.rs`)

- **Settings** are read from the store on every call (`store.all()` → `ConverseSettings` /
  `AwsSettings` / attachment paths, `src-tauri/app/src/settings.rs`), so AWS, inference,
  thinking, guardrail, failover and proxy changes apply to the next call. Exception: MCP URL
  servers use an HTTP client built with the proxy configured at startup (the registry search
  builds a client per call).
- **Errors**: Bedrock failures reject with the `bedrock` crate's error JSON (`name`, `message`,
  `$fault`, `$metadata`; `StructuredOutputError` adds `code` / `details`; `TranslationError`
  adds `code`, `originalText`); tool failures with `{ name, message }`; everything else with the
  TS error message as a plain string.
- **Tools**: `bedrock_execute_tool` / `tools_get_tool_specs` use `tools::full_registry` (every
  ported tool, including MCP, Bedrock, code interpreter and Docker sandbox); the tool context
  carries the chat's `sessionId`, the Docker sandbox, the agent catalog and the sub-agent runner.
  screenCapture / cameraCapture are registered too (see "Screen, camera and task history").
- **Agents**: one `AgentEngine` (Bedrock Converse, full registry, MCP tool specs, background
  session store and listener) is shared by background agents and `sub_agent_invoke`, as
  Electron shared its `BackgroundAgentService`.
- **Background agents**: the scheduler starts at launch when tasks are saved and shuts down on
  exit (with terminal close, sandbox stop and MCP cleanup, like Electron's `before-quit`). Task
  notifications open the task history window when clicked on macOS and Linux (see "Screen,
  camera and task history"); on Windows they come from `tauri-plugin-notification` without a
  click action.
- **Pub/sub**: the subscriber is the calling window's label; its subscriptions are dropped when
  the window is destroyed. Docker sandbox events (`docker-sandbox:state:*`, `:activity:*`,
  `:terminal:*`) are published through the same pub/sub, as in Electron, so only subscribed
  windows receive them.
- **Attachments**: PDF / DOCX text for `chat_attachments_build_context` comes from
  `tools::documents`.
- **Help**: the guide is `docs/USER_GUIDE.md` from the repository in debug builds and the
  bundled `docs/USER_GUIDE.md` resource otherwise.
- `check_docker_availability` returns `lastChecked` as an ISO string; the shim turns it back into
  a `Date`.

Notes on the file commands (`src-tauri/app/src/commands/{file,agent_files}.rs`):

- `open_file` / `open_directory` resolve with the path or `null` (the shim maps `null` to
  `undefined`). Like Electron's `open-directory`, a newly chosen folder is also written to the
  store as `projectPath`; the shim copies it into the store cache.
- `file_read_shared_agents` / `file_read_directory_agents` follow the preload versions (not the
  main-process `read-shared-agents`): file ids are kept, `isShared` + `sharedFilePath` (shared) or
  `directoryOnly` / `isShared: false` / `isCustom: false` (directory) are set. No project path →
  `{ agents: [], error: "Project path not set" }`. Directory agents come from
  `src/renderer/src/assets/directory-agents` in debug builds and the bundled `directory-agents`
  resource otherwise.
- **Chat exports** write to `<projectPath>/<title>/<title>.docx|pdf` (next to the markdown
  export) and resolve with `{ success, filePath?, directory?, error? }`, never rejecting, as in
  Electron. Neither needs Node or Chromium:
  - **Word**: html-to-docx is a JS library, so the shim runs it in the renderer
    (`src/renderer/src/lib/docx/htmlToDocx.ts`, loaded on first use) with the same options and
    the same XML post-processing as the Electron main process, and sends only the finished file:
    `save_chat_to_docx { title, docxBase64 }`. The output is the same document (identical XML
    parts in a Node-free sandbox vs. the Node conversion, apart from html-to-docx's random image
    ids). Its Node
    built-in imports resolve to small browser stand-ins
    (`src/renderer/src/lib/docx/nodeShims/`, Vite aliases in `vite.tauri.config.ts`), and the
    `global` / `Buffer` globals it expects exist only while a conversion runs. Chosen over a Rust
    HTML→docx rewrite because the export HTML is tuned to html-to-docx's quirks.
  - **PDF**: `save_chat_to_pdf { title, html }` prints the self-contained HTML with the system
    webview (`src-tauri/app/src/pdf/`): the document is served from memory over the
    `chatexport` URI scheme into a hidden window, and printed silently to a temp file once it
    has loaded — `NSPrintOperation` on the `WKWebView` (macOS), WebView2 `PrintToPdf`
    (Windows; Chromium, like Electron), WebKitGTK `PrintOperation` to "Print to File" (Linux).
    Same page setup as Electron's `printToPDF` call: US Letter, 1" margins, backgrounds, no
    header/footer; vector text, paginated by the print stylesheet. Chosen over a JS
    HTML→canvas→PDF library (raster pages, no selectable text) and over bundling a headless
    browser. Verified on macOS (WebKit); the Windows code type-checks against `webview2-com`, the
    Linux code against WebKitGTK, but neither has been run.
- `window.open` / `target="_blank"` links open in the default browser (Electron's
  `setWindowOpenHandler` → `shell.openExternal`), via the main window's `on_new_window` handler.
- `window.ipc` is only installed when the name is free: wry defines a read-only `window.ipc`
  (Tauri's postMessage fallback), and assigning to it used to throw and abort the whole install.
  Nothing in the renderer calls `window.ipc` today.

| Bridge method | Command | Args |
| ------------- | ------- | ---- |
| `chatHistory.*` (startup) | `chat_history_snapshot` | — |
| `chatHistory.*` (refresh) | `chat_history_get_all_session_metadata` | — |
| `chatHistory.*` (refresh) | `chat_history_get_recent_sessions` | — |
| `chatHistory.*` (refresh) | `chat_history_get_active_session_id` | — |
| `chatHistory.*` (refresh) | `chat_history_get_session` | `{ sessionId }` |
| `chatHistory.createSession` | `chat_history_create_session` | `{ agentId, modelId, systemPrompt, title }` |
| `chatHistory.addMessage` | `chat_history_add_message` | `{ sessionId, message }` |
| `chatHistory.updateSessionTitle` | `chat_history_update_session_title` | `{ sessionId, title }` |
| `chatHistory.deleteSession` | `chat_history_delete_session` | `{ sessionId }` |
| `chatHistory.deleteSessions` | `chat_history_delete_sessions` | `{ sessionIds }` |
| `chatHistory.deleteAllSessions` | `chat_history_delete_all_sessions` | — |
| `chatHistory.setActiveSession` | `chat_history_set_active_session` | `{ sessionId? }` |
| `chatHistory.updateMessageContent` | `chat_history_update_message_content` | `{ sessionId, messageIndex, updatedMessage }` |
| `chatHistory.deleteMessage` | `chat_history_delete_message` | `{ sessionId, messageIndex }` |
| `file.handleFolderOpen` | `open_directory` | — |
| `file.handleFileOpen` | `open_file` | — |
| `file.readSharedAgents` | `file_read_shared_agents` | — |
| `file.readDirectoryAgents` | `file_read_directory_agents` | — |
| `file.saveSharedAgent` | `save_shared_agent` | `{ agent, options }` |
| `file.deleteSharedAgent` | `delete_shared_agent` | `{ filePath }` |
| `file.exportAgentYaml` | `export_agent_yaml` | `{ agent }` |
| `file.importAgentFile` | `import_agent_file` | — |
| `file.loadOrganizationAgents` | `load_organization_agents` | `{ organizationConfig }` |
| `file.saveAgentToOrganization` | `save_agent_to_organization` | `{ agent, organizationConfig, options }` |
| `file.exportChatMarkdown` | `save_chat_to_markdown` | `(params object as-is)` |
| `file.exportChatDocx` | `save_chat_to_docx` | `{ title, docxBase64 }` (the renderer converts `html`) |
| `file.exportChatPdf` | `save_chat_to_pdf` | `{ title, html }` |
| `api.tools.getToolSpecs` | `tools_get_tool_specs` | — |
| `api.backgroundAgent.chat` | `background_agent_chat` | `(params object as-is)` |
| `api.backgroundAgent.createSession` | `background_agent_create_session` | `{ sessionId, options }` |
| `api.backgroundAgent.deleteSession` | `background_agent_delete_session` | `{ sessionId }` |
| `api.backgroundAgent.listSessions` | `background_agent_list_sessions` | — |
| `api.backgroundAgent.getSessionHistory` | `background_agent_get_session_history` | `{ sessionId }` |
| `api.backgroundAgent.getSessionStats` | `background_agent_get_session_stats` | `{ sessionId }` |
| `api.backgroundAgent.getAllSessionsMetadata` | `background_agent_get_all_sessions_metadata` | — |
| `api.backgroundAgent.getSessionsByProject` | `background_agent_get_sessions_by_project` | `{ projectDirectory }` |
| `api.backgroundAgent.getSessionsByAgent` | `background_agent_get_sessions_by_agent` | `{ agentId }` |
| `api.backgroundAgent.scheduleTask` | `background_agent_schedule_task` | `{ config }` |
| `api.backgroundAgent.updateTask` | `background_agent_update_task` | `{ taskId, config }` |
| `api.backgroundAgent.cancelTask` | `background_agent_cancel_task` | `{ taskId }` |
| `api.backgroundAgent.toggleTask` | `background_agent_toggle_task` | `{ taskId, enabled }` |
| `api.backgroundAgent.listTasks` | `background_agent_list_tasks` | — |
| `api.backgroundAgent.getTask` | `background_agent_get_task` | `{ taskId }` |
| `api.backgroundAgent.getTaskExecutionHistory` | `background_agent_get_task_execution_history` | `{ taskId }` |
| `api.backgroundAgent.executeTaskManually` | `background_agent_execute_task_manually` | `{ taskId }` |
| `api.backgroundAgent.getSchedulerStats` | `background_agent_get_scheduler_stats` | — |
| `api.backgroundAgent.continueSession` | `background_agent_continue_session` | `(params object as-is)` |
| `api.backgroundAgent.getTaskSystemPrompt` | `background_agent_get_task_system_prompt` | `{ taskId }` |
| `api.bedrock.executeTool` | `bedrock_execute_tool` | `{ toolInput, context }` |
| `api.bedrock.applyGuardrail` | `bedrock_apply_guardrail` | `{ request }` |
| `api.bedrock.listApplicationInferenceProfiles` | `bedrock_list_application_inference_profiles` | — |
| `api.bedrock.translateText` | `bedrock_translate_text` | `(params object as-is)` |
| `api.bedrock.translateBatch` | `bedrock_translate_batch` | `{ texts }` |
| `api.bedrock.getCachedTranslation` | `bedrock_get_translation_cache` | `(params object as-is)` |
| `api.bedrock.clearTranslationCache` | `bedrock_clear_translation_cache` | — |
| `api.bedrock.getTranslationCacheStats` | `bedrock_get_translation_cache_stats` | — |
| `api.bedrock.getModelMaxTokens` | `bedrock_get_model_max_tokens` | `{ modelId }` |
| `api.images.getLocalImage` | `get_local_image` | `{ path }` |
| `api.openDirectory` | `open_directory` | — |
| `api.readProjectIgnore` | `read_project_ignore` | `{ projectPath }` |
| `api.writeProjectIgnore` | `write_project_ignore` | `{ projectPath, content }` |
| `api.mcp.init` | `mcp_init` | `{ mcpServers }` |
| `api.mcp.getToolSpecs` | `mcp_get_tools` | `{ mcpServers }` |
| `api.mcp.executeTool` | `mcp_execute_tool` | `{ toolName, input, mcpServers }` |
| `api.mcp.testConnection` | `mcp_test_connection` | `{ mcpServer }` |
| `api.mcp.testAllConnections` | `mcp_test_all_connections` | `{ mcpServers }` |
| `api.mcp.searchRegistry` | `mcp_search_registry` | `{ query, limit }` |
| `api.mcp.cleanup` | `mcp_cleanup` | — |
| `api.codeInterpreter.checkDockerAvailability` | `check_docker_availability` | — |
| `api.dockerSandbox.availability` | `docker_sandbox_availability` | `{ force }` |
| `api.dockerSandbox.create` | `docker_sandbox_create` | `{ sessionId, options }` |
| `api.dockerSandbox.status` | `docker_sandbox_status` | `{ sessionId }` |
| `api.dockerSandbox.start` | `docker_sandbox_start` | `{ sessionId }` |
| `api.dockerSandbox.stop` | `docker_sandbox_stop` | `{ sessionId }` |
| `api.dockerSandbox.remove` | `docker_sandbox_remove` | `{ sessionId, options }` |
| `api.dockerSandbox.rename` | `docker_sandbox_rename` | `{ sessionId }` |
| `api.dockerSandbox.logs` | `docker_sandbox_logs` | `{ sessionId, ...options }` |
| `api.dockerSandbox.exec` | `docker_sandbox_exec` | `{ sessionId, command, options }` |
| `api.dockerSandbox.sendInput` | `docker_sandbox_send_input` | `{ pid, stdin }` |
| `api.dockerSandbox.hasPid` | `docker_sandbox_has_pid` | `{ pid }` |
| `api.dockerSandbox.list` | `docker_sandbox_list` | — |
| `api.dockerSandbox.openFolder` | `docker_sandbox_open_folder` | `{ sessionId }` |
| `api.dockerSandbox.openPort` | `docker_sandbox_open_port` | `{ sessionId, port }` |
| `api.dockerSandbox.insights` | `docker_sandbox_insights` | `{ sessionId, service }` |
| `api.dockerSandbox.compose` | `docker_sandbox_compose` | `{ sessionId }` |
| `api.dockerSandbox.activity` | `docker_sandbox_activity` | `{ sessionId }` |
| `api.dockerSandbox.terminal.capability` | `docker_sandbox_terminal_capability` | — |
| `api.dockerSandbox.terminal.open` | `docker_sandbox_terminal_open` | `{ sessionId, ...options }` |
| `api.dockerSandbox.terminal.attach` | `docker_sandbox_terminal_attach` | `{ terminalId }` |
| `api.dockerSandbox.terminal.input` | `docker_sandbox_terminal_input` | `{ terminalId, data }` |
| `api.dockerSandbox.terminal.resize` | `docker_sandbox_terminal_resize` | `{ terminalId, cols, rows }` |
| `api.dockerSandbox.terminal.backlog` | `docker_sandbox_terminal_backlog` | `{ terminalId }` |
| `api.dockerSandbox.terminal.close` | `docker_sandbox_terminal_close` | `{ terminalId }` |
| `api.chatAttachments.list` | `chat_attachments_list` | `{ sessionId }` |
| `api.chatAttachments.withFiles` | `chat_attachments_with_files` | `{ sessionIds }` |
| `api.chatAttachments.add` | `chat_attachments_add` | `{ sessionId, files: [{ name, bytes: number[] }] }` |
| `api.chatAttachments.addFromPicker` | `chat_attachments_add_from_picker` | `{ sessionId }` |
| `api.chatAttachments.remove` | `chat_attachments_remove` | `{ sessionId, name }` |
| `api.chatAttachments.removeAll` | `chat_attachments_remove_all` | `{ sessionId }` |
| `api.chatAttachments.removeEveryFolder` | `chat_attachments_remove_every_folder` | — |
| `api.chatAttachments.rename` | `chat_attachments_rename` | `{ sessionId }` |
| `api.chatAttachments.buildContext` | `chat_attachments_build_context` | `{ sessionId }` |
| `api.chatAttachments.openFolder` | `chat_attachments_open_folder` | `{ sessionId }` |
| `api.help.prepareUserGuide` | `help_prepare_user_guide` | `{ sessionId }` |
| `api.screen.listAvailableWindows` | `screen_list_available_windows` | — |
| `api.camera.saveCapturedImage` | `camera_save_captured_image` | `{ request }` |
| `api.camera.showPreviewWindow` | `camera_show_preview_window` | `{ options }` |
| `api.camera.hidePreviewWindow` | `camera_hide_preview_window` | — |
| `api.camera.closePreviewWindow` | `camera_close_preview_window` | `{ deviceId }` |
| `api.camera.updatePreviewSettings` | `camera_update_preview_settings` | `{ options }` |
| `api.camera.getPreviewStatus` | `camera_get_preview_status` | — |
| `api.pubsub.subscribe` | `pubsub_subscribe` | `{ channel }` |
| `api.pubsub.unsubscribe` | `pubsub_unsubscribe` | `{ channel }` |
| `api.pubsub.publish` | `pubsub_publish` | `{ channel, data }` |
| `api.pubsub.stats` | `pubsub_stats` | — |
| `api.window.openTaskHistory` | `window_open_task_history` | `{ taskId }` |
| `lib/nativeMenus.ts` (right-click, main window) | `context_menu_popup` | — (Copy/Paste menu at the cursor; Electron's `context-menu` handler) |
| `lib/nativeMenus.ts` (Cmd/Ctrl+`+`; on Windows also `=` `-` `0` `r`) | `app_menu_action` | `{ action: 'zoomIn' \| 'zoomOut' \| 'resetZoom' \| 'reload' }` (Electron's `before-input-event` keys) |
| `api.todo.getTodoList` | `get_todo_list` | `(params object as-is)` |
| `api.todo.initTodoList` | `todo_init` | `(params object as-is)` |
| `api.todo.updateTodoList` | `todo_update` | `(params object as-is)` |
| `api.todo.deleteTodoList` | `delete_todo_list` | `(params object as-is)` |
| `api.todo.getRecentTodos` | `get_recent_todos` | — |
| `api.todo.getAllTodoMetadata` | `get_all_todo_metadata` | — |
| `api.todo.setActiveTodoList` | `set_active_todo_list` | `(params object as-is)` |
| `api.todo.getActiveTodoListId` | `get_active_todo_list_id` | — |
| `api.strandsConverter.convertAndSave` | `convert_agent_to_strands` | `{ agentId, outputDirectory }` → `SaveResult` (`src-tauri/crates/strands`), or `{ success: false, error }` for an unknown agent |
| `api.subAgent.invoke` | `sub_agent_invoke` | `(params object as-is)` |
| `lib/api.ts converse()` | `converse` | `{ request }` |
| `lib/api.ts streamChatCompletion()` | `converse_stream` | `{ streamId, request, onEvent }` |
| `lib/api.ts` (abort) | `converse_cancel` | `{ streamId }` |
| `lib/api.ts retrieveAndGenerate()` | `retrieve_and_generate` | `{ request }` |
| `lib/api.ts listModels()` | `list_models` | — |
| `lib/api.ts listAgentTags()` | `list_agent_tags` | — |
| `lib/api.ts getStructuredOutput()` | `get_structured_output` | `{ request }` |
| `lib/api.ts getWebsiteRecommendations()` | `get_website_recommendations` | `{ request }` |

## Converse (replaces the Express routes)

`src/renderer/src/lib/api.ts` used to `fetch` the in-process Express server
(`src/main/api/index.ts`). Under Tauri each function invokes a command instead; the exported
function signatures, including the `streamChatCompletion` async generator, are unchanged.

| Express route | Command | Request | Result |
| ------------- | ------- | ------- | ------ |
| `POST /converse/stream` | `converse_stream` | `{ streamId, request: CallConverseAPIProps, onEvent: Channel }` | resolves with the **number of events sent** (`u32`) |
| `POST /converse` | `converse` | `{ request: CallConverseAPIProps, requestId?: string }` | `ConverseCommandOutput` JSON |
| — | `converse_cancel` | `{ streamId }` (a stream id or a `converse` request id) | `()`; an id that isn't running yet is remembered, see Cancellation |
| `POST /retrieveAndGenerate` | `retrieve_and_generate` | `{ request: RetrieveAndGenerateCommandInput }` | `RetrieveAndGenerateCommandOutput` JSON (wrapped in a `Response` for callers) |
| `GET /listModels` | `list_models` | — | `LLM[]` (`BedrockService.listModels()`) |
| `GET /listAgentTags` | `list_agent_tags` | — | `string[]` (the Express route never existed; nothing calls it) |
| `POST /structured-output` | `get_structured_output` | `{ request: { modelId, systemPrompt, userMessage, outputSchema, toolOptions?, inferenceConfig? } }` | the structured object |
| `POST /website-recommendations` | `get_website_recommendations` | `{ request: { websiteCode, language, modelId } }` | `{ recommendations: { title, value }[] }` |

Nova Sonic (`/nova-sonic/region-check`, Socket.IO) and `/bedrock/connectivity-test` are dropped
with the voice chat UI.

### Request: `CallConverseAPIProps`

Same JSON the renderer POSTed before (`src/main/api/bedrock/types.ts`):

```ts
{
  modelId: string
  messages: Message[]          // Bedrock Converse messages
  system: SystemContentBlock[] // [{ text }] plus optional { cachePoint: { type: 'default' } }
  toolConfig?: ToolConfiguration   // tools may include { cachePoint } entries
  guardrailConfig?: GuardrailConfiguration
  inferenceConfig?: InferenceConfiguration // omitted → store `inferenceParams`
  disableThinking?: boolean
}
```

Encodings that differ from the SDK types because they went through JSON:

- Image blocks carry base64 strings: `{ image: { format, source: { bytes: "<base64>" } } }`.
  `processImageContent` (`src/main/api/bedrock/utils/imageUtils.ts`) decoded them in main, and
  also accepted the `{"0": n, "1": n, …}` form a JSON-serialized `Uint8Array` produces; Rust
  should accept both.
- `reasoningContent.redactedContent` is whatever the stream emitted for it (see below), echoed
  back verbatim on the next turn.

The Rust side must apply the same request building as `ConverseService.buildCommandParams`
(`src/main/api/bedrock/services/converseService.ts`): inference params and thinking mode from the
store, interleaved thinking, model-specific tweaks (Nova, Kimi, …), message sanitization, guardrail
from `guardrailSettings` when none is given, and the region failover in `handleError`.

### Stream events (`onEvent` channel messages)

Each channel message is **one `ConverseStreamOutput` union member serialized as JSON**, exactly the
object the Express route wrote per line (`JSON.stringify(item)` over the SDK stream). Only one key
is present per event. Field names are the SDK's camelCase:

```jsonc
{ "messageStart": { "role": "assistant" } }
{ "contentBlockStart": { "contentBlockIndex": 0, "start": { "toolUse": { "toolUseId": "tooluse_…", "name": "readFiles" } } } }
{ "contentBlockDelta": { "contentBlockIndex": 0, "delta": { "text": "Hel" } } }
{ "contentBlockDelta": { "contentBlockIndex": 1, "delta": { "toolUse": { "input": "{\"paths\":" } } } }
{ "contentBlockDelta": { "contentBlockIndex": 0, "delta": { "reasoningContent": { "text": "…" } } } }
{ "contentBlockDelta": { "contentBlockIndex": 0, "delta": { "reasoningContent": { "signature": "…" } } } }
{ "contentBlockDelta": { "contentBlockIndex": 0, "delta": { "reasoningContent": { "redactedContent": "<base64>" } } } }
{ "contentBlockStop": { "contentBlockIndex": 0 } }
{ "messageStop": { "stopReason": "end_turn" | "tool_use" | "max_tokens" | "stop_sequence" | "guardrail_intervened" | "content_filtered", "additionalModelResponseFields": { … } } }
{ "metadata": { "usage": { "inputTokens": 1, "outputTokens": 2, "totalTokens": 3, "cacheReadInputTokens": 0, "cacheWriteInputTokens": 0 }, "metrics": { "latencyMs": 123 }, "trace": { … } } }
```

- Omit absent optional fields rather than sending `null` (the SDK JSON never had nulls).
  `contentBlockStart.start` is absent for text blocks.
- `redactedContent` was a `Uint8Array` in the SDK (serialized as `{"0":n,…}` by Express, which was
  never meaningful). Rust sends a base64 string and must accept it back in request messages.
- Consumers: `pages/ChatPage/hooks/useAgentChat.ts` (reads `messageStart.role`,
  `contentBlockStart.start.toolUse`, `contentBlockDelta.delta.{text,toolUse.input,reasoningContent}`,
  `messageStop.stopReason`, and stores `metadata` as `converseMetadata`) and `hooks/useChat.ts`.
- Stream exceptions (`internalServerException`, `modelStreamErrorException`,
  `throttlingException`, `validationException`, `serviceUnavailableException`) are **not** sent as
  events: the SDK threw them, so the command returns `Err` with
  `{"name":"<ExceptionName>","message":"…","eventsSent":<n>}`, where `eventsSent` is the number
  of channel events sent before the failure. Errors before the stream starts (validation,
  credentials, access denied) are returned the same way (`eventsSent: 0`). The generator then throws an `Error`
  with that `name`/`message`, which the chat UI shows as a toast and an assistant message.
- **Completion:** the command resolves only after its last `Channel::send`, with the count of
  events sent. Channel messages and the invoke response travel separately, so the shim keeps
  reading until it has received that many events (on success) or `eventsSent` events (on a
  non-abort error, which it throws only after yielding them). If they haven't all arrived within
  a grace period (`converseStreamOptions.tailGraceMs`, 5 s) after the command settles, the shim
  logs it and throws the command error, or on success an `Error` saying how many of how many
  events arrived, rather than waiting forever.
- **Cancellation:** the renderer generates `streamId` (`stream-<crypto.randomUUID()>`, unique
  across windows; Rust rejects an id that is already running with a plain error). When the caller's
  `AbortSignal` fires, or the consumer stops iterating early, the shim calls
  `converse_cancel { streamId }` and throws `AbortError` (callers check `error.name ===
  'AbortError'`). Rust should drop the Bedrock stream for that id and return promptly from
  `converse_stream` (`Ok(count)` or an error; the renderer ignores the result after an abort).
  Async command arguments are deserialized inside the spawned task, so a `converse_cancel` sent
  right after the invoke can reach Rust before the request has registered its token: Rust then
  remembers the id (bounded, 2 minutes) and the request returns `AbortError` as soon as it starts,
  without calling Bedrock. `converse` takes the same kind of id as `requestId`
  (`converse-<uuid>`); `api.ts converse()` sends one and calls `converse_cancel { streamId:
  requestId }` when its signal aborts, so the backend request stops too (Electron only discarded
  the result).
- **Failures of `converse`** reject with the same error JSON as `converse_stream`. (Under Express
  `POST /converse` answered 500 with the error object and `res.json()` *resolved* with it, so
  callers failed later on `result.output`; they now get a named `Error` instead.)
- Implemented in `src-tauri/app/src/commands/bedrock.rs` over `bedrock::ConverseService`
  (request building, retries and region failover live there). Verified against Bedrock with the
  `bedrock` crate's `converse_stream_smoke` integration test: text blocks start with a
  `contentBlockDelta` (no `contentBlockStart`), as documented above.
- When `supportsStreamingWithToolUse(modelId)` is false and tools are present, `api.ts` still calls
  `converse` and synthesizes the events itself (`converseOutputToStreamEvents`); Rust doesn't need
  to handle that case. Each content block gets its own `contentBlockIndex`, and a tool use is
  `contentBlockStart { toolUseId, name }` + a `contentBlockDelta` with the JSON-stringified input +
  `contentBlockStop`, as in a real stream. (Electron sent the input only in `start` and every block
  at index 0, so the chat built tool calls with input `{}`.)

## Security

`src-tauri/app/src/security.rs` defines the app origins (`tauri://localhost`,
`http(s)://tauri.localhost`, and `build.devUrl` in debug builds) used by all of the below.

- **Command ACL.** `build.rs` reads the `generate_handler![...]` list in
  `src-tauri/app/src/commands/mod.rs` (`src/command_list.rs`), passes it to
  `tauri_build::AppManifest::commands` (which generates `allow-<command>` / `deny-<command>`
  under `permissions/autogenerated/`, gitignored) and writes `permissions/app-commands.toml`, a set
  granting all of them. With an app manifest Tauri checks every app command against the
  capabilities: `capabilities/default.json` gives `main` and `task-history` `app-commands`;
  `camera-preview.json` gives previews only their two commands; `pdf-export-*` windows match no
  capability, so they have no IPC at all. Adding a command to `handler()` is all that's needed
  for the main windows; `commands/acl_tests.rs` fails if the capabilities name unknown commands or
  if `camera-preview.html` starts invoking one its capability lacks.
- **Content-Security-Policy** (`app.security.csp` in `tauri.conf.json`; Tauri adds it to every
  HTML page it serves, i.e. production builds — `tauri dev` loads the Vite server directly and
  gets none, so check CSP changes with a `--features tauri/custom-protocol` build):
  `script-src 'self'` (no remote script, no `eval`), `style-src 'self' 'unsafe-inline'`
  (Tailwind/emotion/sandpack/monaco/mermaid inject styles; `dangerousDisableAssetCspModification:
  ["style-src"]` keeps Tauri from adding nonces there, which would disable `'unsafe-inline'`),
  `img-src 'self' data: blob: https:` (chat markdown, Tavily result images and GitHub avatars are
  remote), `font-src 'self' data:`, `connect-src 'self' ipc: http://ipc.localhost chathistory:
  http://chathistory.localhost`, `frame-src htmlpreview: http://htmlpreview.localhost
  https://*.codesandbox.io https://embed.diagrams.net` (HTML previews, Website Generator,
  draw.io), `worker-src 'self' blob:`, `object-src 'none'`, `base-uri 'self'`,
  `form-action 'self'`, `frame-ancestors 'none'`. Consequences: monaco is bundled
  (`lib/monaco/setup.ts`) instead of loaded from jsDelivr; `vite.tauri.config.ts` rewrites
  dependencies' `eval("this")` / `new Function("return this")()` to `globalThis`. Violations are
  logged once each (category `security:csp`) by `lib/cspViolationLogger.ts`.
- **HTML previews** (```html blocks, `HtmlBlock.tsx`): an iframe sandboxed with `allow-scripts`
  only (opaque origin: no parent DOM, storage, `__TAURI_INTERNALS__`, and IPC requests carry
  `Origin: null`, which Tauri rejects). A `srcdoc` iframe would inherit the app CSP, so it loads
  the `htmlpreview://localhost/preview` shell (`src-tauri/app/src/html_preview.rs`, its own
  permissive CSP plus `sandbox allow-scripts`), which announces `html-preview:ready` and renders
  the `html-preview:render { html }` message the parent answers with.
- **Other embedded content**: sandpack's bundler iframe (`*.codesandbox.io`, `allow-same-origin`
  refers to its own origin) and draw.io (`embed.diagrams.net`) are cross-origin frames; Tauri
  injects its IPC globals into the main frame only. Mermaid runs with `securityLevel:
  'antiscript'` (its SVG is inserted into the app document); `JSONViewer` HTML-escapes the data
  before highlighting.
- **Navigation.** The main and task history windows have an `on_navigation` guard: app pages,
  `about:`/`blob:`, the preview scheme and the embed hosts load (on macOS the handler also sees
  iframe navigations); other `http(s)`/`mailto` links open in the default browser; anything else
  is cancelled. `window.open` / `target="_blank"` still go to the browser via `on_new_window`.

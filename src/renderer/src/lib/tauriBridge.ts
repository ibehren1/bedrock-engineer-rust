/**
 * Tauri bridge shim.
 *
 * Installs `window.api`, `window.store`, `window.file`, `window.chatHistory`, `window.appWindow`,
 * `window.ipc` and `window.logger` (typed in src/types/window.ts), backed by `invoke()` / events.
 * These are the objects the Electron preload script used to expose, with the same shapes, so the
 * renderer components were ported unchanged.
 *
 * `installTauriBridge()` must be awaited before any module that touches `window.*` is evaluated
 * (main.tsx imports the app dynamically after it resolves): `window.store.get`,
 * `window.api.tools.getToolSpecs` and the chat-history getters are synchronous, so their data is
 * hydrated into in-memory caches first.
 *
 * ## Command naming scheme
 *
 * Full table: docs/port/BRIDGE.md. The rules, so a later task can add the matching
 * `#[tauri::command]` without reading this file:
 *
 * 1. Methods that were `ipcRenderer.invoke(channel, ...)` in the preload use the Electron channel
 *    name converted to snake_case: `:` and `-` become `_`, camelCase is split.
 *    `background-agent:chat` → `background_agent_chat`, `window:openTaskHistory` →
 *    `window_open_task_history`, `get-local-image` → `get_local_image`.
 * 2. Methods the preload implemented itself (no IPC channel) use `<namespace>_<method>` in
 *    snake_case: `window.api.bedrock.executeTool` → `bedrock_execute_tool`,
 *    `window.chatHistory.createSession` → `chat_history_create_session`,
 *    `window.file.readSharedAgents` → `file_read_shared_agents`.
 * 3. Arguments are always a named object. When the Electron channel took a single params object,
 *    that object is passed as-is (its keys become the Rust parameter names; Tauri maps camelCase
 *    keys to snake_case parameters). Positional arguments are wrapped with the parameter names from
 *    the preload signature (`getLocalImage(path)` → `{ path }`).
 * 4. Commands return `Result<T, String>`. The error string may be a plain message or a JSON object
 *    `{"name": "...", "message": "..."}`; both become an `Error` with that name/message.
 * 5. Main → renderer pushes (`ipcRenderer.on(channel)`) become Tauri events with the same name
 *    (`background-agent:task-notification`, `context-menu-command`). Pub/sub channels are emitted
 *    as events named by `pubsubEventName(channel)`.
 *
 * Chat streaming (`tauriConverseStream`, used by lib/api.ts) is at the bottom of this file; its
 * event contract is in docs/port/BRIDGE.md, "Converse".
 *
 * A command that is not registered yet rejects with `Error("not yet ported: <method>
 * (<command>)")` and is logged once; nothing throws synchronously, so the UI still loads.
 */
import { invoke, Channel, convertFileSrc } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getImageGenerationModelsForRegion } from '@common/models/models'
import { captureCameraFrame, type CameraFrameRequest } from './cameraCapture'
import type { ChatMessage, ChatSession, SessionMetadata } from '@/types/chat/history'

// ---------------------------------------------------------------------------------------------
// invoke helpers
// ---------------------------------------------------------------------------------------------

/** Error raised when a bridge method's Rust command does not exist yet. */
export class NotPortedError extends Error {
  constructor(
    public readonly method: string,
    public readonly command: string
  ) {
    super(`not yet ported: ${method} (${command})`)
    this.name = 'NotPortedError'
  }
}

const warnedNotPorted = new Set<string>()

function isUnknownCommand(err: unknown, command: string): boolean {
  const msg = typeof err === 'string' ? err : (err as any)?.message
  return typeof msg === 'string' && msg.includes(command) && /not found/i.test(msg)
}

/** Turn whatever a Tauri command rejected with into an `Error`. */
export function toError(err: unknown): Error {
  if (err instanceof Error) return err
  if (typeof err === 'string') {
    try {
      const parsed = JSON.parse(err)
      if (parsed && typeof parsed === 'object' && typeof parsed.message === 'string') {
        const e = new Error(parsed.message)
        if (typeof parsed.name === 'string') e.name = parsed.name
        return Object.assign(e, parsed)
      }
    } catch {
      // plain string
    }
    return new Error(err)
  }
  if (err && typeof err === 'object' && typeof (err as any).message === 'string') {
    const e = new Error((err as any).message)
    if (typeof (err as any).name === 'string') e.name = (err as any).name
    return Object.assign(e, err)
  }
  return new Error(String(err))
}

/**
 * `invoke` with error normalization and "not yet ported" detection.
 * @param method the bridge path, used in messages (e.g. `api.bedrock.translateText`)
 */
export async function call<T = any>(
  method: string,
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  try {
    // The command reads settings on the Rust side; let this window's queued store writes land.
    if (SETTINGS_DEPENDENT_COMMAND.test(command)) await storeFlush()
    return await invoke<T>(command, args)
  } catch (err) {
    if (isUnknownCommand(err, command)) {
      if (!warnedNotPorted.has(command)) {
        warnedNotPorted.add(command)
        console.warn(`[tauriBridge] not yet ported: ${method} → invoke('${command}')`)
      }
      throw new NotPortedError(method, command)
    }
    throw toError(err)
  }
}

/** Build a bridge method that forwards to `command`, mapping positional args to named ones. */
const cmd =
  (
    method: string,
    command: string,
    mapArgs?: (...a: any[]) => Record<string, unknown> | undefined
  ) =>
  (...a: any[]): any =>
    call(method, command, mapArgs ? mapArgs(...a) : undefined)

/**
 * Like `cmd`, but for preload methods that never reject and instead return an error-shaped value
 * (`{ success: false, error }`). Keeps that contract when the command is missing or fails.
 */
const cmdResult =
  (
    method: string,
    command: string,
    mapArgs: ((...a: any[]) => Record<string, unknown> | undefined) | undefined,
    onError: (e: Error) => any
  ) =>
  async (...a: any[]): Promise<any> => {
    try {
      return await call(method, command, mapArgs ? mapArgs(...a) : undefined)
    } catch (e) {
      console.error(`Error in ${method}:`, e)
      return onError(toError(e))
    }
  }

const failResult = (e: Error) => ({ success: false, error: e.message })

/** A sync method with no Rust equivalent yet: log once, return a neutral value. */
function unportedSync<T>(method: string, fallback: T): () => T {
  return () => {
    if (!warnedNotPorted.has(method)) {
      warnedNotPorted.add(method)
      console.warn(`[tauriBridge] not yet ported (sync): ${method}`)
    }
    return fallback
  }
}

// ---------------------------------------------------------------------------------------------
// events
// ---------------------------------------------------------------------------------------------

/** Subscribe to a Tauri event, returning a synchronous cleanup like `ipcRenderer.removeListener`. */
function on<T = any>(event: string, callback: (payload: T) => void): () => void {
  let unlisten: UnlistenFn | undefined
  let cancelled = false
  listen<T>(event, (e) => callback(e.payload))
    .then((fn) => {
      if (cancelled) fn()
      else unlisten = fn
    })
    .catch((e) => console.warn(`[tauriBridge] listen(${event}) failed`, e))
  return () => {
    cancelled = true
    unlisten?.()
  }
}

/**
 * Tauri event names allow only alphanumerics and `-/:_`. Pub/sub channel names are arbitrary, so
 * anything else becomes `_`. Rust must emit pub/sub messages under this same name.
 */
export const pubsubEventName = (channel: string): string =>
  `pubsub:${channel.replace(/[^A-Za-z0-9\-/:_]/g, '_')}`

// ---------------------------------------------------------------------------------------------
// window.store — sync cache hydrated from `store_all`, write-through via `store_set`/`store_delete`
// ---------------------------------------------------------------------------------------------

let storeCache: Record<string, unknown> = {}
let storeWrites: Promise<unknown> = Promise.resolve()
/**
 * False until `store_all` has succeeded. Until then `store.set` is refused: writing defaults over
 * settings that merely failed to load (e.g. `customAgents = []`) would destroy them.
 */
let storeHydrated = false
/** Keys with a local write still queued or in flight; `store-changed` echoes for them are ignored. */
const pendingStoreKeys = new Map<string, number>()

const clone = <T>(v: T): T => (v === undefined ? v : structuredClone(v))

/** Raised by `installTauriBridge()` when the settings could not be loaded. */
export class StoreHydrationError extends Error {
  constructor(public readonly reason: unknown) {
    super(`Could not load settings: ${toError(reason).message}`)
    this.name = 'StoreHydrationError'
  }
}

/** Resolves once every `store.set` issued so far has reached the backend (or failed). */
export function storeFlush(): Promise<void> {
  return storeWrites.then(() => undefined)
}

function createStore(): Window['store'] {
  return {
    // electron-store reads from disk on every get, so callers always receive a fresh object;
    // cloning keeps in-place mutation of a returned value from silently changing the cache.
    get: (key: any): any => clone(storeCache[key]),
    set: (key: any, value: any): any => {
      if (!storeHydrated) {
        console.error(`[tauriBridge] store.set('${String(key)}') ignored: settings were not loaded`)
        return
      }
      const removing = value === undefined
      // Snapshot now: the caller may mutate `value` before the queued write runs.
      const sent = clone(value)
      if (removing) delete storeCache[key]
      else storeCache[key] = clone(value)
      const k = String(key)
      pendingStoreKeys.set(k, (pendingStoreKeys.get(k) ?? 0) + 1)
      // Serialize writes so the file ends up in call order.
      storeWrites = storeWrites
        .then(() =>
          removing
            ? call('store.set', 'store_delete', { key })
            : call('store.set', 'store_set', { key, value: sent })
        )
        .catch((e) => console.error(`[tauriBridge] store.set('${String(key)}') failed`, e))
        .finally(() => {
          const n = (pendingStoreKeys.get(k) ?? 1) - 1
          if (n > 0) pendingStoreKeys.set(k, n)
          else pendingStoreKeys.delete(k)
        })
    }
  }
}

/**
 * `store-changed` from Rust after any config store write (this or another window, or Rust
 * itself): keep this window's cache in step. While this window has its own write to the key
 * queued, the event is older than the local value and is ignored.
 */
export function applyStoreChange(change: { key: string; value?: unknown; deleted?: boolean }) {
  if (!change || typeof change.key !== 'string') return
  if (pendingStoreKeys.has(change.key)) return
  if (change.deleted) delete storeCache[change.key]
  else storeCache[change.key] = change.value
}

/** Answer Rust's pre-close / pre-quit `store-flush-request` once queued writes have landed. */
function installStoreFlushResponder() {
  on<{ token: number }>('store-flush-request', async (payload) => {
    await storeFlush()
    await call('store', 'store_flush_done', { token: payload?.token ?? 0 }).catch((e) =>
      console.error('[tauriBridge] store_flush_done failed', e)
    )
  })
}

/** Commands that read settings from the store on the Rust side: wait for queued writes first. */
const SETTINGS_DEPENDENT_COMMAND =
  /^(tools_execute$|converse$|converse_stream$|bedrock_|background_agent_)/

// ---------------------------------------------------------------------------------------------
// window.logger — forwards to `logger_log` (Electron: ipcRenderer.send('logger:log'))
// ---------------------------------------------------------------------------------------------

let loggerAvailable = true

function sendLog(level: string, message: string, meta: Record<string, any> = {}) {
  const entry = {
    level,
    message,
    timestamp: new Date().toISOString(),
    process: 'renderer',
    ...meta
  }
  if (!loggerAvailable) return
  call('logger.log', 'logger_log', { entry }).catch((e) => {
    // Don't retry a missing command on every log line.
    if (e instanceof NotPortedError) loggerAvailable = false
  })
}

function createLogger(): Window['logger'] {
  const make = (extra: Record<string, any>) => ({
    error: (m: string, meta: Record<string, any> = {}) =>
      sendLog('error', m, { ...meta, ...extra }),
    warn: (m: string, meta: Record<string, any> = {}) => sendLog('warn', m, { ...meta, ...extra }),
    info: (m: string, meta: Record<string, any> = {}) => sendLog('info', m, { ...meta, ...extra }),
    debug: (m: string, meta: Record<string, any> = {}) =>
      sendLog('debug', m, { ...meta, ...extra }),
    verbose: (m: string, meta: Record<string, any> = {}) =>
      sendLog('verbose', m, { ...meta, ...extra })
  })
  return {
    log: make({}),
    createCategoryLogger: (category: string) => make({ category })
  }
}

// ---------------------------------------------------------------------------------------------
// window.chatHistory — sync getters served from a cache hydrated by `chat_history_snapshot`
// ---------------------------------------------------------------------------------------------

type ChatHistorySnapshot = {
  activeSessionId?: string | null
  metadata: SessionMetadata[]
  recent: SessionMetadata[]
  sessions: Record<string, ChatSession>
}

const chat: ChatHistorySnapshot = {
  activeSessionId: undefined,
  metadata: [],
  recent: [],
  sessions: {}
}

async function hydrateChatHistory() {
  try {
    const snap = await call<ChatHistorySnapshot>('chatHistory', 'chat_history_snapshot')
    chat.activeSessionId = snap.activeSessionId ?? undefined
    chat.metadata = snap.metadata ?? []
    chat.recent = snap.recent ?? []
    chat.sessions = snap.sessions ?? {}
  } catch (e) {
    if (!(e instanceof NotPortedError))
      console.error('[tauriBridge] chat history hydrate failed', e)
  }
}

/** Re-read the list-level state (and optionally one session) after a mutation. */
async function refreshChat(sessionId?: string) {
  const M = 'chatHistory.refresh'
  const [metadata, recent, active, session] = await Promise.all([
    call<SessionMetadata[]>(M, 'chat_history_get_all_session_metadata'),
    call<SessionMetadata[]>(M, 'chat_history_get_recent_sessions'),
    call<string | null>(M, 'chat_history_get_active_session_id'),
    sessionId
      ? call<ChatSession | null>(M, 'chat_history_get_session', { sessionId })
      : Promise.resolve(undefined)
  ])
  chat.metadata = metadata ?? []
  chat.recent = recent ?? []
  chat.activeSessionId = active ?? undefined
  if (sessionId) {
    if (session) chat.sessions[sessionId] = session
    else delete chat.sessions[sessionId]
  }
}

/**
 * `getSession` is synchronous but the snapshot only carries the active and recent sessions. For any
 * other id, read it with a synchronous XHR from the `chathistory` URI scheme the Rust side serves
 * (`convertFileSrc(id, 'chathistory')` → `chathistory://localhost/<id>`, or
 * `http://chathistory.localhost/<id>` on Windows). `null` when the session doesn't exist.
 */
function fetchSessionSync(sessionId: string): ChatSession | null {
  try {
    const xhr = new XMLHttpRequest()
    xhr.open('GET', convertFileSrc(sessionId, 'chathistory'), false)
    xhr.send()
    if (xhr.status !== 200) return null
    const session = JSON.parse(xhr.responseText) as ChatSession | null
    if (session) chat.sessions[sessionId] = session
    return session
  } catch (e) {
    console.error(`[tauriBridge] chatHistory.getSession('${sessionId}') failed`, e)
    return null
  }
}

function background(method: string, p: Promise<unknown>) {
  p.catch((e) => console.error(`[tauriBridge] ${method} failed`, e))
}

function createChatHistory(): Window['chatHistory'] {
  const M = 'chatHistory'
  return {
    async createSession(agentId: string, modelId: string, systemPrompt?: string) {
      const id = await call<string>(`${M}.createSession`, 'chat_history_create_session', {
        agentId,
        modelId,
        systemPrompt,
        // The preload built the title in the renderer's locale; keep that.
        title: `Chat ${new Date().toLocaleString()}`
      })
      await refreshChat(id)
      return id
    },
    async addMessage(sessionId: string, message: ChatMessage) {
      await call(`${M}.addMessage`, 'chat_history_add_message', { sessionId, message })
      await refreshChat(sessionId)
    },
    getSession(sessionId: string) {
      if (!sessionId) return null
      return clone(chat.sessions[sessionId] ?? fetchSessionSync(sessionId))
    },
    async updateSessionTitle(sessionId: string, title: string) {
      await call(`${M}.updateSessionTitle`, 'chat_history_update_session_title', {
        sessionId,
        title
      })
      await refreshChat(sessionId)
    },
    deleteSession(sessionId: string) {
      delete chat.sessions[sessionId]
      chat.metadata = chat.metadata.filter((m) => m.id !== sessionId)
      chat.recent = chat.recent.filter((m) => m.id !== sessionId)
      background(
        `${M}.deleteSession`,
        call(`${M}.deleteSession`, 'chat_history_delete_session', { sessionId }).then(() =>
          refreshChat()
        )
      )
    },
    deleteSessions(sessionIds: string[]) {
      const ids = new Set(sessionIds)
      ids.forEach((id) => delete chat.sessions[id])
      chat.metadata = chat.metadata.filter((m) => !ids.has(m.id))
      chat.recent = chat.recent.filter((m) => !ids.has(m.id))
      background(
        `${M}.deleteSessions`,
        call(`${M}.deleteSessions`, 'chat_history_delete_sessions', { sessionIds }).then(() =>
          refreshChat()
        )
      )
    },
    deleteAllSessions() {
      chat.sessions = {}
      chat.metadata = []
      chat.recent = []
      chat.activeSessionId = undefined
      background(
        `${M}.deleteAllSessions`,
        call(`${M}.deleteAllSessions`, 'chat_history_delete_all_sessions').then(() => refreshChat())
      )
    },
    getRecentSessions() {
      return clone(chat.recent)
    },
    getAllSessionMetadata() {
      return clone(chat.metadata)
    },
    setActiveSession(sessionId: string | undefined) {
      chat.activeSessionId = sessionId
      background(
        `${M}.setActiveSession`,
        call(`${M}.setActiveSession`, 'chat_history_set_active_session', { sessionId })
      )
    },
    getActiveSessionId() {
      return chat.activeSessionId ?? undefined
    },
    async updateMessageContent(
      sessionId: string,
      messageIndex: number,
      updatedMessage: ChatMessage
    ) {
      await call(`${M}.updateMessageContent`, 'chat_history_update_message_content', {
        sessionId,
        messageIndex,
        updatedMessage
      })
      await refreshChat(sessionId)
    },
    async deleteMessage(sessionId: string, messageIndex: number) {
      await call(`${M}.deleteMessage`, 'chat_history_delete_message', { sessionId, messageIndex })
      await refreshChat(sessionId)
    }
  } as Window['chatHistory']
}

// ---------------------------------------------------------------------------------------------
// window.file
// ---------------------------------------------------------------------------------------------

/**
 * `open_directory`. Like Electron's `open-directory`, choosing a new folder also stores it as
 * `projectPath` on the Rust side; mirror that into the store cache so `store.get` sees it.
 */
const openDirectory = (method: string) => async (): Promise<string | undefined> => {
  const path = await call<string | null>(method, 'open_directory')
  if (!path) return undefined
  storeCache.projectPath = path
  return path
}

function createFile(): Window['file'] {
  const F = 'file'
  const agentsError = (e: Error) => ({ agents: [], error: e })
  return {
    handleFolderOpen: openDirectory(`${F}.handleFolderOpen`),
    handleFileOpen: async () =>
      (await call<string | null>(`${F}.handleFileOpen`, 'open_file')) ?? undefined,
    readSharedAgents: cmdResult(
      `${F}.readSharedAgents`,
      'file_read_shared_agents',
      undefined,
      agentsError
    ),
    readDirectoryAgents: cmdResult(
      `${F}.readDirectoryAgents`,
      'file_read_directory_agents',
      undefined,
      agentsError
    ),
    saveSharedAgent: cmdResult(
      `${F}.saveSharedAgent`,
      'save_shared_agent',
      (agent, options) => ({ agent, options }),
      failResult
    ),
    deleteSharedAgent: cmdResult(
      `${F}.deleteSharedAgent`,
      'delete_shared_agent',
      (filePath) => ({ filePath }),
      failResult
    ),
    exportAgentYaml: cmdResult(
      `${F}.exportAgentYaml`,
      'export_agent_yaml',
      (agent) => ({ agent }),
      failResult
    ),
    importAgentFile: cmdResult(`${F}.importAgentFile`, 'import_agent_file', undefined, failResult),
    loadOrganizationAgents: cmdResult(
      `${F}.loadOrganizationAgents`,
      'load_organization_agents',
      (organizationConfig) => ({ organizationConfig }),
      (e) => ({ agents: [], error: e.message })
    ),
    saveAgentToOrganization: cmdResult(
      `${F}.saveAgentToOrganization`,
      'save_agent_to_organization',
      (agent, organizationConfig, options) => ({ agent, organizationConfig, options }),
      failResult
    ),
    exportChatMarkdown: cmdResult(
      `${F}.exportChatMarkdown`,
      'save_chat_to_markdown',
      (data) => data,
      failResult
    ),
    // html-to-docx runs here (Tauri has no Node process); Rust only writes the bytes.
    exportChatDocx: async (data: { title: string; html: string }) => {
      const method = `${F}.exportChatDocx`
      try {
        // Loaded on first use, to keep html-to-docx out of the startup bundle.
        // eslint-disable-next-line no-restricted-syntax
        const { convertChatHtmlToDocx, bytesToBase64 } = await import('./docx/htmlToDocx')
        const docx = await convertChatHtmlToDocx(data.html)
        return await call(method, 'save_chat_to_docx', {
          title: data.title,
          docxBase64: bytesToBase64(docx)
        })
      } catch (e) {
        console.error(`Error in ${method}:`, e)
        return failResult(toError(e))
      }
    },
    exportChatPdf: cmdResult(`${F}.exportChatPdf`, 'save_chat_to_pdf', (data) => data, failResult)
  }
}

// ---------------------------------------------------------------------------------------------
// window.api
// ---------------------------------------------------------------------------------------------

let toolSpecsCache: any[] = []

async function hydrateToolSpecs() {
  try {
    toolSpecsCache = (await call<any[]>('api.tools.getToolSpecs', 'tools_get_tool_specs')) ?? []
  } catch (e) {
    if (!(e instanceof NotPortedError)) console.error('[tauriBridge] tool specs hydrate failed', e)
  }
}

async function isWindowFocused(): Promise<boolean> {
  try {
    return await getCurrentWindow().isFocused()
  } catch {
    return document.hasFocus()
  }
}

/** Unwrap the `{ success, error, ...payload }` envelope the MCP IPC handlers return. */
async function unwrap<T>(p: Promise<any>, pick?: (r: any) => T): Promise<T> {
  const result = await p
  if (!result?.success) throw new Error(result?.error)
  return pick ? pick(result) : result
}

/** Uint8Array does not survive JSON; send bytes as number[] (serde `Vec<u8>`). */
const bytesToArray = (b: Uint8Array | number[]) => (Array.isArray(b) ? b : Array.from(b))

/** The reverse, for byte results (`{ backlog: number[] }` → `{ backlog: Uint8Array }`). */
const withBacklogBytes = async (p: Promise<any>) => {
  const r = await p
  return r && Array.isArray(r.backlog) ? { ...r, backlog: Uint8Array.from(r.backlog) } : r
}

/** `lastChecked` was a `Date` over Electron IPC; Rust sends its ISO string. */
const withLastCheckedDate = async (p: Promise<any>) => {
  const r = await p
  return r && typeof r.lastChecked === 'string' ? { ...r, lastChecked: new Date(r.lastChecked) } : r
}

function createApi(): Window['api'] {
  const BA = 'api.backgroundAgent'
  const B = 'api.bedrock'
  const DS = 'api.dockerSandbox'
  const T = 'api.dockerSandbox.terminal'
  const CA = 'api.chatAttachments'
  const CAM = 'api.camera'
  const TD = 'api.todo'
  const MCP = 'api.mcp'

  // Active pub/sub listeners per channel, so `unsubscribe(channel)` can drop them all like
  // `ipcRenderer.removeAllListeners(channel)`.
  const pubsubListeners = new Map<string, Set<() => void>>()

  return {
    backgroundAgent: {
      chat: cmd(`${BA}.chat`, 'background_agent_chat', (params) => params),
      onTaskNotification: (callback: (params: any) => void) =>
        on('background-agent:task-notification', callback),
      onTaskExecutionStart: (callback: (params: any) => void) =>
        on('background-agent:task-execution-start', callback),
      onTaskSkipped: (callback: (params: any) => void) =>
        on('background-agent:task-skipped', callback),
      createSession: cmd(
        `${BA}.createSession`,
        'background_agent_create_session',
        (sessionId, options) => ({
          sessionId,
          options
        })
      ),
      deleteSession: cmd(`${BA}.deleteSession`, 'background_agent_delete_session', (sessionId) => ({
        sessionId
      })),
      listSessions: cmd(`${BA}.listSessions`, 'background_agent_list_sessions'),
      getSessionHistory: cmd(
        `${BA}.getSessionHistory`,
        'background_agent_get_session_history',
        (sessionId) => ({
          sessionId
        })
      ),
      getSessionStats: cmd(
        `${BA}.getSessionStats`,
        'background_agent_get_session_stats',
        (sessionId) => ({
          sessionId
        })
      ),
      getAllSessionsMetadata: cmd(
        `${BA}.getAllSessionsMetadata`,
        'background_agent_get_all_sessions_metadata'
      ),
      getSessionsByProject: cmd(
        `${BA}.getSessionsByProject`,
        'background_agent_get_sessions_by_project',
        (projectDirectory) => ({ projectDirectory })
      ),
      getSessionsByAgent: cmd(
        `${BA}.getSessionsByAgent`,
        'background_agent_get_sessions_by_agent',
        (agentId) => ({
          agentId
        })
      ),
      scheduleTask: cmd(`${BA}.scheduleTask`, 'background_agent_schedule_task', (config) => ({
        config
      })),
      updateTask: cmd(`${BA}.updateTask`, 'background_agent_update_task', (taskId, config) => ({
        taskId,
        config
      })),
      cancelTask: cmd(`${BA}.cancelTask`, 'background_agent_cancel_task', (taskId) => ({ taskId })),
      toggleTask: cmd(`${BA}.toggleTask`, 'background_agent_toggle_task', (taskId, enabled) => ({
        taskId,
        enabled
      })),
      listTasks: cmd(`${BA}.listTasks`, 'background_agent_list_tasks'),
      getTask: cmd(`${BA}.getTask`, 'background_agent_get_task', (taskId) => ({ taskId })),
      getTaskExecutionHistory: cmd(
        `${BA}.getTaskExecutionHistory`,
        'background_agent_get_task_execution_history',
        (taskId) => ({ taskId })
      ),
      executeTaskManually: cmd(
        `${BA}.executeTaskManually`,
        'background_agent_execute_task_manually',
        (taskId) => ({
          taskId
        })
      ),
      getSchedulerStats: cmd(`${BA}.getSchedulerStats`, 'background_agent_get_scheduler_stats'),
      continueSession: cmd(
        `${BA}.continueSession`,
        'background_agent_continue_session',
        (params) => params
      ),
      getTaskSystemPrompt: cmd(
        `${BA}.getTaskSystemPrompt`,
        'background_agent_get_task_system_prompt',
        (taskId) => ({ taskId })
      )
    },
    bedrock: {
      executeTool: cmd(`${B}.executeTool`, 'bedrock_execute_tool', (toolInput, context) => ({
        toolInput,
        context
      })),
      applyGuardrail: cmd(`${B}.applyGuardrail`, 'bedrock_apply_guardrail', (request) => ({
        request
      })),
      // Pure lookup over the shared model table; no backend needed.
      getImageGenerationModelsForRegion: (region: any) => getImageGenerationModelsForRegion(region),
      listApplicationInferenceProfiles: cmd(
        `${B}.listApplicationInferenceProfiles`,
        'bedrock_list_application_inference_profiles'
      ),
      // Pure mapping, duplicated from InferenceProfileService.convertProfileToLLM so it can stay
      // synchronous. Keep in step with that function.
      convertInferenceProfileToLLM: (profile: any) => {
        const arn: string = profile?.modelSource?.copyFrom
        const parts = arn ? arn.split('/') : []
        const baseModelId = (arn && parts[parts.length - 1]) || 'unknown'
        return {
          modelId: profile.inferenceProfileArn,
          modelName: profile.inferenceProfileName || `Inference Profile: ${baseModelId}`,
          toolUse: true,
          regions: ['us-east-1', 'us-west-2'],
          isInferenceProfile: true,
          inferenceProfileArn: profile.inferenceProfileArn,
          maxTokensLimit: 4096,
          supportsThinking: false,
          description: profile.description
        }
      },
      translateText: cmd(`${B}.translateText`, 'bedrock_translate_text', (params) => params),
      translateBatch: cmd(`${B}.translateBatch`, 'bedrock_translate_batch', (texts) => ({ texts })),
      getCachedTranslation: cmd(
        `${B}.getCachedTranslation`,
        'bedrock_get_translation_cache',
        (params) => params
      ),
      clearTranslationCache: cmd(`${B}.clearTranslationCache`, 'bedrock_clear_translation_cache'),
      getTranslationCacheStats: cmd(
        `${B}.getTranslationCacheStats`,
        'bedrock_get_translation_cache_stats'
      ),
      getModelMaxTokens: cmd(
        `${B}.getModelMaxTokens`,
        'bedrock_get_model_max_tokens',
        (modelId) => ({
          modelId
        })
      )
    },
    contextMenu: {
      onContextMenuCommand: (callback: (command: string) => void) => {
        on<string>('context-menu-command', callback)
      }
    },
    images: {
      getLocalImage: cmd('api.images.getLocalImage', 'get_local_image', (path) => ({ path }))
    },
    openDirectory: openDirectory('api.openDirectory'),
    readProjectIgnore: cmd('api.readProjectIgnore', 'read_project_ignore', (projectPath) => ({
      projectPath
    })),
    writeProjectIgnore: cmd(
      'api.writeProjectIgnore',
      'write_project_ignore',
      (projectPath, content) => ({
        projectPath,
        content
      })
    ),
    mcp: {
      init: (mcpServers: any[]) => unwrap(call(`${MCP}.init`, 'mcp_init', { mcpServers })),
      getToolSpecs: (mcpServers: any[]) =>
        unwrap(call(`${MCP}.getToolSpecs`, 'mcp_get_tools', { mcpServers }), (r) => r.tools),
      executeTool: cmd(`${MCP}.executeTool`, 'mcp_execute_tool', (toolName, input, mcpServers) => ({
        toolName,
        input,
        mcpServers
      })),
      testConnection: cmd(`${MCP}.testConnection`, 'mcp_test_connection', (mcpServer) => ({
        mcpServer
      })),
      testAllConnections: (mcpServers: any[]) =>
        unwrap(
          call(`${MCP}.testAllConnections`, 'mcp_test_all_connections', { mcpServers }),
          (r) => r.results
        ),
      searchRegistry: (query: string, limit?: number) =>
        unwrap(
          call(`${MCP}.searchRegistry`, 'mcp_search_registry', { query, limit }),
          (r) => r.servers
        ),
      cleanup: () => unwrap(call(`${MCP}.cleanup`, 'mcp_cleanup'))
    },
    codeInterpreter: {
      getCurrentWorkspacePath: unportedSync<string | null>(
        'api.codeInterpreter.getCurrentWorkspacePath',
        null
      ),
      checkDockerAvailability: () =>
        withLastCheckedDate(
          call('api.codeInterpreter.checkDockerAvailability', 'check_docker_availability')
        )
    },
    dockerSandbox: {
      availability: cmd(`${DS}.availability`, 'docker_sandbox_availability', (force) => ({
        force
      })),
      create: cmd(`${DS}.create`, 'docker_sandbox_create', (sessionId, options) => ({
        sessionId,
        options
      })),
      status: cmd(`${DS}.status`, 'docker_sandbox_status', (sessionId) => ({ sessionId })),
      start: cmd(`${DS}.start`, 'docker_sandbox_start', (sessionId) => ({ sessionId })),
      stop: cmd(`${DS}.stop`, 'docker_sandbox_stop', (sessionId) => ({ sessionId })),
      remove: cmd(`${DS}.remove`, 'docker_sandbox_remove', (sessionId, options) => ({
        sessionId,
        options
      })),
      rename: cmd(`${DS}.rename`, 'docker_sandbox_rename', (sessionId) => ({ sessionId })),
      logs: cmd(`${DS}.logs`, 'docker_sandbox_logs', (sessionId, options) => ({
        sessionId,
        ...options
      })),
      exec: cmd(`${DS}.exec`, 'docker_sandbox_exec', (sessionId, command, options) => ({
        sessionId,
        command,
        options
      })),
      sendInput: cmd(`${DS}.sendInput`, 'docker_sandbox_send_input', (pid, stdin) => ({
        pid,
        stdin
      })),
      hasPid: cmd(`${DS}.hasPid`, 'docker_sandbox_has_pid', (pid) => ({ pid })),
      list: cmd(`${DS}.list`, 'docker_sandbox_list'),
      openFolder: cmd(`${DS}.openFolder`, 'docker_sandbox_open_folder', (sessionId) => ({
        sessionId
      })),
      openPort: cmd(`${DS}.openPort`, 'docker_sandbox_open_port', (sessionId, port) => ({
        sessionId,
        port
      })),
      insights: cmd(`${DS}.insights`, 'docker_sandbox_insights', (sessionId, service) => ({
        sessionId,
        service
      })),
      compose: cmd(`${DS}.compose`, 'docker_sandbox_compose', (sessionId) => ({ sessionId })),
      activity: cmd(`${DS}.activity`, 'docker_sandbox_activity', (sessionId) => ({ sessionId })),
      terminal: {
        capability: cmd(`${T}.capability`, 'docker_sandbox_terminal_capability'),
        open: cmd(`${T}.open`, 'docker_sandbox_terminal_open', (sessionId, options) => ({
          sessionId,
          ...options
        })),
        // Live `data` frames carry `bytes: number[]`, which `new Uint8Array(bytes)` accepts as
        // is; the backlog is checked with `byteLength`, so it must be a real Uint8Array.
        attach: (terminalId: string) =>
          withBacklogBytes(call(`${T}.attach`, 'docker_sandbox_terminal_attach', { terminalId })),
        input: cmd(`${T}.input`, 'docker_sandbox_terminal_input', (terminalId, data) => ({
          terminalId,
          data
        })),
        resize: cmd(`${T}.resize`, 'docker_sandbox_terminal_resize', (terminalId, cols, rows) => ({
          terminalId,
          cols,
          rows
        })),
        backlog: (terminalId: string) =>
          withBacklogBytes(call(`${T}.backlog`, 'docker_sandbox_terminal_backlog', { terminalId })),
        close: cmd(`${T}.close`, 'docker_sandbox_terminal_close', (terminalId) => ({ terminalId }))
      }
    },
    chatAttachments: {
      list: cmd(`${CA}.list`, 'chat_attachments_list', (sessionId) => ({ sessionId })),
      withFiles: cmd(`${CA}.withFiles`, 'chat_attachments_with_files', (sessionIds) => ({
        sessionIds
      })),
      add: cmd(
        `${CA}.add`,
        'chat_attachments_add',
        (sessionId, files: { name: string; bytes: Uint8Array }[]) => ({
          sessionId,
          files: files.map((f) => ({ name: f.name, bytes: bytesToArray(f.bytes) }))
        })
      ),
      addFromPicker: cmd(
        `${CA}.addFromPicker`,
        'chat_attachments_add_from_picker',
        (sessionId) => ({
          sessionId
        })
      ),
      remove: cmd(`${CA}.remove`, 'chat_attachments_remove', (sessionId, name) => ({
        sessionId,
        name
      })),
      removeAll: cmd(`${CA}.removeAll`, 'chat_attachments_remove_all', (sessionId) => ({
        sessionId
      })),
      removeEveryFolder: cmd(`${CA}.removeEveryFolder`, 'chat_attachments_remove_every_folder'),
      rename: cmd(`${CA}.rename`, 'chat_attachments_rename', (sessionId) => ({ sessionId })),
      buildContext: cmd(`${CA}.buildContext`, 'chat_attachments_build_context', (sessionId) => ({
        sessionId
      })),
      openFolder: cmd(`${CA}.openFolder`, 'chat_attachments_open_folder', (sessionId) => ({
        sessionId
      }))
    },
    help: {
      prepareUserGuide: cmd(
        'api.help.prepareUserGuide',
        'help_prepare_user_guide',
        (sessionId) => ({
          sessionId
        })
      )
    },
    screen: {
      listAvailableWindows: cmd('api.screen.listAvailableWindows', 'screen_list_available_windows')
    },
    camera: {
      saveCapturedImage: cmd(
        `${CAM}.saveCapturedImage`,
        'camera_save_captured_image',
        (request) => ({
          request
        })
      ),
      showPreviewWindow: cmd(
        `${CAM}.showPreviewWindow`,
        'camera_show_preview_window',
        (options) => ({
          options
        })
      ),
      hidePreviewWindow: cmd(`${CAM}.hidePreviewWindow`, 'camera_hide_preview_window'),
      closePreviewWindow: cmd(
        `${CAM}.closePreviewWindow`,
        'camera_close_preview_window',
        (deviceId) => ({
          deviceId
        })
      ),
      updatePreviewSettings: cmd(
        `${CAM}.updatePreviewSettings`,
        'camera_update_preview_settings',
        (options) => ({
          options
        })
      ),
      getPreviewStatus: cmd(`${CAM}.getPreviewStatus`, 'camera_get_preview_status')
    },
    tools: {
      getToolSpecs: () => toolSpecsCache
    },
    pubsub: {
      subscribe: (channel: string, callback: (data: any) => void) => {
        const off = on(pubsubEventName(channel), callback)
        let set = pubsubListeners.get(channel)
        if (!set) pubsubListeners.set(channel, (set = new Set()))
        set.add(off)
        background(
          'api.pubsub.subscribe',
          call('api.pubsub.subscribe', 'pubsub_subscribe', { channel })
        )
        return () => {
          off()
          pubsubListeners.get(channel)?.delete(off)
          background(
            'api.pubsub.unsubscribe',
            call('api.pubsub.unsubscribe', 'pubsub_unsubscribe', { channel })
          )
        }
      },
      unsubscribe: (channel: string) => {
        pubsubListeners.get(channel)?.forEach((off) => off())
        pubsubListeners.delete(channel)
        return call('api.pubsub.unsubscribe', 'pubsub_unsubscribe', { channel })
      },
      publish: cmd('api.pubsub.publish', 'pubsub_publish', (channel, data) => ({ channel, data })),
      stats: cmd('api.pubsub.stats', 'pubsub_stats')
    },
    window: {
      isFocused: isWindowFocused,
      openTaskHistory: cmd('api.window.openTaskHistory', 'window_open_task_history', (taskId) => ({
        taskId
      }))
    },
    todo: {
      getTodoList: cmd(`${TD}.getTodoList`, 'get_todo_list', (params) => params ?? {}),
      initTodoList: cmd(`${TD}.initTodoList`, 'todo_init', (params) => params),
      updateTodoList: cmd(`${TD}.updateTodoList`, 'todo_update', (params) => params),
      deleteTodoList: cmd(`${TD}.deleteTodoList`, 'delete_todo_list', (params) => params),
      getRecentTodos: cmd(`${TD}.getRecentTodos`, 'get_recent_todos'),
      getAllTodoMetadata: cmd(`${TD}.getAllTodoMetadata`, 'get_all_todo_metadata'),
      setActiveTodoList: cmd(`${TD}.setActiveTodoList`, 'set_active_todo_list', (params) => params),
      getActiveTodoListId: cmd(`${TD}.getActiveTodoListId`, 'get_active_todo_list_id')
    },
    strandsConverter: {
      convertAndSave: cmd(
        'api.strandsConverter.convertAndSave',
        'convert_agent_to_strands',
        (agentId, outputDirectory) => ({ agentId, outputDirectory })
      )
    },
    subAgent: {
      invoke: cmd('api.subAgent.invoke', 'sub_agent_invoke', (params) => params)
    }
  }
}

// ---------------------------------------------------------------------------------------------
// cameraCapture frames (Rust tool → main window getUserMedia → camera_capture_response)
// ---------------------------------------------------------------------------------------------

/**
 * The cameraCapture tool runs in Rust but the camera is only reachable from a webview, so Rust
 * emits `camera:capture-request { requestId, deviceId?, quality, format }` to the main window
 * and waits for `camera_capture_response { requestId, frame }` or `{ requestId, error }`.
 */
function installCameraCaptureResponder() {
  on<CameraFrameRequest>('camera:capture-request', async (request) => {
    const requestId = request?.requestId
    let answer: Record<string, unknown>
    try {
      answer = { requestId, frame: await captureCameraFrame(request) }
    } catch (e) {
      answer = { requestId, error: e instanceof Error ? e.message : String(e) }
    }
    call('camera', 'camera_capture_response', answer).catch((e) =>
      console.error('[tauriBridge] camera_capture_response failed', e)
    )
  })
}

// ---------------------------------------------------------------------------------------------
// window.ipc — generic typed channel call (rule 1 applied at runtime)
// ---------------------------------------------------------------------------------------------

/** Electron IPC channel → Tauri command name (see rule 1 above). */
export const channelToCommand = (channel: string): string =>
  channel
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[:-]/g, '_')
    .toLowerCase()

const isPlainObject = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === 'object' && !Array.isArray(v) && Object.getPrototypeOf(v) === Object.prototype

function createIpc(): Window['ipc'] {
  return {
    invoke: ((channel: string, ...args: any[]) =>
      call(
        `ipc.invoke('${channel}')`,
        channelToCommand(channel),
        args.length === 0
          ? undefined
          : args.length === 1 && isPlainObject(args[0])
            ? args[0]
            : { args }
      )) as Window['ipc']['invoke']
  }
}

// ---------------------------------------------------------------------------------------------
// install
// ---------------------------------------------------------------------------------------------

let installed: Promise<void> | undefined

/**
 * Install the bridge and hydrate its synchronous caches. Safe to call more than once.
 */
export function installTauriBridge(): Promise<void> {
  if (installed) return installed
  installed = (async () => {
    const w = window as any
    w.store = createStore()
    w.logger = createLogger()
    w.chatHistory = createChatHistory()
    w.file = createFile()
    w.api = createApi()
    // wry already defines a read-only `window.ipc` (Tauri's postMessage IPC fallback). Assigning
    // to it throws, which used to abort the install before any cache was hydrated. Nothing in the
    // renderer uses `window.ipc` today, so leave wry's object alone and expose ours only when the
    // name is free.
    if (!('ipc' in w)) w.ipc = createIpc()
    w.appWindow = { isFocused: isWindowFocused }
    // Only the main window answers cameraCapture frame requests (the task history window loads
    // the bridge too).
    if (getCurrentWindow().label === 'main') installCameraCaptureResponder()

    if (!document.title) document.title = __APP_NAME__

    // Store changes made elsewhere (other windows, Rust). Ones arriving before hydration are
    // replayed on top of the snapshot, in order.
    const earlyChanges: Parameters<typeof applyStoreChange>[0][] = []
    on<Parameters<typeof applyStoreChange>[0]>('store-changed', (change) =>
      storeHydrated ? applyStoreChange(change) : earlyChanges.push(change)
    )
    installStoreFlushResponder()

    const [all] = await Promise.all([
      call<Record<string, unknown>>('store', 'store_all').then(
        (value) =>
          value && typeof value === 'object' && !Array.isArray(value)
            ? { ok: true as const, value }
            : { ok: false as const, error: new Error('store_all returned no settings object') },
        (error) => ({ ok: false as const, error })
      ),
      hydrateChatHistory(),
      hydrateToolSpecs()
    ])
    if (!all.ok) {
      // Don't start the app on an empty settings cache: the first settings save would write
      // defaults over the real values (custom agents, credentials, ...). main.tsx shows an error
      // screen with a retry instead, and store.set stays disabled.
      console.error('[tauriBridge] store_all failed', all.error)
      throw new StoreHydrationError(all.error)
    }
    storeCache = all.value as Record<string, unknown>
    storeHydrated = true
    earlyChanges.splice(0).forEach(applyStoreChange)
  })()
  return installed
}

// ---------------------------------------------------------------------------------------------
// converse (replaces the Express /converse and /converse/stream routes)
// ---------------------------------------------------------------------------------------------

/**
 * How long the stream shim waits, after `converse_stream` settles, for Channel events still in
 * flight. Mutable for tests.
 */
export const converseStreamOptions = { tailGraceMs: 5000 }

let requestCounter = 0

/** A request id unique across windows (each window has its own counters and clock reads). */
const newRequestId = (prefix: string) => {
  const uuid = globalThis.crypto?.randomUUID?.()
  return `${prefix}-${uuid ?? `${Date.now()}-${++requestCounter}-${Math.random().toString(36).slice(2)}`}`
}

const abortError = () => {
  const e = new Error('The operation was aborted.')
  e.name = 'AbortError'
  return e
}

/**
 * Stream `converse_stream` events as an async iterable. Each yielded value is one
 * `ConverseStreamOutput` member object, exactly what the Express route wrote per line.
 * Aborting the signal calls `converse_cancel` and throws an `AbortError`, like `fetch` does.
 */
export async function* tauriConverseStream<T = any>(
  request: unknown,
  abortSignal?: AbortSignal
): AsyncGenerator<T, void, unknown> {
  if (abortSignal?.aborted) throw abortError()

  const streamId = newRequestId('stream')
  const queue: T[] = []
  let received = 0
  // Number of events the command reports having sent (the resolved value, or `eventsSent` on the
  // error); set when it settles.
  let expected: number | undefined
  // The command's error, thrown once the events sent before it have been yielded.
  let failure: Error | undefined
  let aborted: Error | undefined
  let tailTimedOut = false
  let tailTimer: ReturnType<typeof setTimeout> | undefined
  let wake: (() => void) | undefined
  const notify = () => {
    wake?.()
    wake = undefined
  }

  const onEvent = new Channel<T>()
  onEvent.onmessage = (event) => {
    received++
    queue.push(event)
    notify()
  }

  const onAbort = () => {
    aborted = abortError()
    notify()
    call('converse', 'converse_cancel', { streamId }).catch(() => {})
  }
  abortSignal?.addEventListener('abort', onAbort, { once: true })

  // Channel messages and the invoke response travel separately, so the response (or rejection)
  // can overtake the last few events. The command reports how many it sent; the shim keeps
  // reading until it has that many, giving up after `tailGraceMs` if some never arrive.
  const settle = (count: number) => {
    expected = count
    if (received < count) {
      tailTimer = setTimeout(() => {
        tailTimedOut = true
        notify()
      }, converseStreamOptions.tailGraceMs)
    }
    notify()
  }
  call<number>('converse', 'converse_stream', { streamId, request, onEvent }).then(
    (count) => settle(typeof count === 'number' ? count : received),
    (e) => {
      failure = toError(e)
      const sent = (failure as { eventsSent?: unknown }).eventsSent
      settle(typeof sent === 'number' ? sent : received)
    }
  )

  try {
    while (true) {
      if (aborted) throw aborted
      if (queue.length > 0) {
        yield queue.shift() as T
        continue
      }
      if (expected !== undefined && (received >= expected || tailTimedOut)) {
        if (received < expected) {
          const msg = `Converse stream ${streamId} ended with ${received} of ${expected} events received`
          console.error(`[tauriBridge] ${msg}`)
          if (!failure) throw new Error(msg)
        }
        if (failure) throw failure
        return
      }
      await new Promise<void>((resolve) => (wake = resolve))
    }
  } finally {
    abortSignal?.removeEventListener('abort', onAbort)
    clearTimeout(tailTimer)
    if (expected === undefined && !aborted) {
      // Consumer stopped early (break/return): stop the backend stream too.
      call('converse', 'converse_cancel', { streamId }).catch(() => {})
    }
  }
}

/**
 * Invoke non-streaming `converse`. Aborting the signal rejects with `AbortError` and calls
 * `converse_cancel` with the request id, so the backend stops too.
 */
export async function tauriConverse<T = any>(
  request: unknown,
  abortSignal?: AbortSignal
): Promise<T> {
  if (abortSignal?.aborted) throw abortError()
  const requestId = newRequestId('converse')
  const onAbort = () => {
    call('converse', 'converse_cancel', { streamId: requestId }).catch(() => {})
  }
  abortSignal?.addEventListener('abort', onAbort, { once: true })
  try {
    return await withAbort(call<T>('converse', 'converse', { request, requestId }), abortSignal)
  } finally {
    abortSignal?.removeEventListener('abort', onAbort)
  }
}

/** Race a promise against an AbortSignal, rejecting with `AbortError` like `fetch`. */
export function withAbort<T>(p: Promise<T>, abortSignal?: AbortSignal): Promise<T> {
  if (!abortSignal) return p
  if (abortSignal.aborted) return Promise.reject(abortError())
  return new Promise<T>((resolve, reject) => {
    const onAbort = () => reject(abortError())
    abortSignal.addEventListener('abort', onAbort, { once: true })
    p.then(resolve, reject).finally(() => abortSignal.removeEventListener('abort', onAbort))
  })
}

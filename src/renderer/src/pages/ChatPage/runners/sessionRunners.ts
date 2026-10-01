import type { IdentifiableMessage } from '@/types/chat/message'
import type { ToolName } from '@/types/tools'

/**
 * 進行中のエージェントターンの状態をセッション単位でモジュールスコープに保持する。
 * React の外に置くことで、別のチャットに切り替えたり ChatPage をアンマウントしても
 * ターンが中断されず、戻ってきたときに続きが見える。
 *
 * Agent turns used to live in `useAgentChat`'s React state, so there was room for exactly
 * one conversation per mounted hook and switching sessions had to abort it. This registry
 * holds that state per session outside React instead: a turn keeps running while the user
 * reads another chat or another page, and `useAgentChat` just subscribes to the entry for
 * whichever session is on screen.
 *
 * A run survives session switches and navigation, but not a window reload — the loop itself
 * still runs in the renderer. Moving it to the main process is the next step up from here.
 */

/** Everything `useAgentChat` renders for one session. */
export interface SessionRunState {
  messages: IdentifiableMessage[]
  loading: boolean
  reasoning: boolean
  waitingForResponse: boolean
  timeoutCountdown: number
  heartbeatCount: number
  executingTools: Set<ToolName>
  latestReasoningText: string
}

/**
 * Shared snapshot for sessions with no runner yet. A stable identity matters:
 * `useSyncExternalStore` re-renders in a loop if `getSnapshot` returns a new object.
 */
const EMPTY_STATE: SessionRunState = Object.freeze({
  messages: [],
  loading: false,
  reasoning: false,
  waitingForResponse: false,
  timeoutCountdown: 0,
  heartbeatCount: 0,
  executingTools: new Set<ToolName>(),
  latestReasoningText: ''
}) as SessionRunState

interface RunnerEntry {
  /** Replaced wholesale on every change, so subscribers can compare by identity. */
  state: SessionRunState
  /** In-flight request for this session, or null when idle. */
  abortController: AbortController | null
  /** Index of the first prompt-cache point to send with this session's next request. */
  lastCachePoint: number | undefined
  /** Assistant message the next `metadata` stream event belongs to. */
  lastAssistantMessageId: string | null
  listeners: Set<() => void>
}

const runners = new Map<string, RunnerEntry>()

/** Subscribers to "which sessions are running", for the history list's indicator. */
const runningListeners = new Set<() => void>()
let runningSnapshot: readonly string[] = []

function createEntry(): RunnerEntry {
  return {
    state: EMPTY_STATE,
    abortController: null,
    lastCachePoint: undefined,
    lastAssistantMessageId: null,
    listeners: new Set()
  }
}

function ensureEntry(key: string): RunnerEntry {
  let entry = runners.get(key)
  if (!entry) {
    entry = createEntry()
    runners.set(key, entry)
  }
  return entry
}

function refreshRunningSnapshot(): void {
  const next = [...runners.entries()]
    .filter(([, entry]) => entry.state.loading)
    .map(([key]) => key)
    .sort()
  const changed =
    next.length !== runningSnapshot.length || next.some((id, i) => id !== runningSnapshot[i])
  if (changed) {
    runningSnapshot = next
    runningListeners.forEach((listener) => listener())
  }
}

function commit(entry: RunnerEntry, next: SessionRunState): void {
  entry.state = next
  entry.listeners.forEach((listener) => listener())
  refreshRunningSnapshot()
}

// ---------------------------------------------------------------------------
// Reading state
// ---------------------------------------------------------------------------

/** Current snapshot for a session. Stable identity until something changes. */
export function getRunState(key?: string): SessionRunState {
  if (!key) return EMPTY_STATE
  return runners.get(key)?.state ?? EMPTY_STATE
}

/** Subscribe to one session's state. Returns the unsubscribe function. */
export function subscribeToRunState(key: string | undefined, listener: () => void): () => void {
  if (!key) return () => {}
  const entry = ensureEntry(key)
  entry.listeners.add(listener)
  return () => {
    entry.listeners.delete(listener)
  }
}

/** Whether a turn is in flight for this session. */
export function isSessionRunning(key?: string): boolean {
  return !!key && !!runners.get(key)?.state.loading
}

/** Ids of every session with a turn in flight. Stable identity until the set changes. */
export function getRunningSessionIds(): readonly string[] {
  return runningSnapshot
}

/** Subscribe to the set of running sessions. Returns the unsubscribe function. */
export function subscribeToRunningSessions(listener: () => void): () => void {
  runningListeners.add(listener)
  return () => {
    runningListeners.delete(listener)
  }
}

// ---------------------------------------------------------------------------
// Writing state
// ---------------------------------------------------------------------------

/** Merge a partial update into a session's state. */
export function patchRunState(key: string, patch: Partial<SessionRunState>): void {
  const entry = ensureEntry(key)
  commit(entry, { ...entry.state, ...patch })
}

/** Update a single field with a function of its previous value. */
export function updateRunField<K extends keyof SessionRunState>(
  key: string,
  field: K,
  update: (previous: SessionRunState[K]) => SessionRunState[K]
): void {
  const entry = ensureEntry(key)
  commit(entry, { ...entry.state, [field]: update(entry.state[field]) })
}

/** Replace a session's messages, either directly or from the previous array. */
export function setRunMessages(
  key: string,
  next: IdentifiableMessage[] | ((previous: IdentifiableMessage[]) => IdentifiableMessage[])
): void {
  const entry = ensureEntry(key)
  const messages = typeof next === 'function' ? next(entry.state.messages) : next
  commit(entry, { ...entry.state, messages })
}

/**
 * Load a session's messages from the store when opening it. A no-op while a turn is in
 * flight, because the runner's messages are newer than anything on disk.
 */
export function seedRunMessages(key: string, messages: IdentifiableMessage[]): void {
  if (isSessionRunning(key)) return
  const entry = ensureEntry(key)
  commit(entry, { ...EMPTY_STATE, messages })
  entry.lastCachePoint = undefined
  entry.lastAssistantMessageId = null
}

// ---------------------------------------------------------------------------
// Per-run scratch state (was refs on the hook)
// ---------------------------------------------------------------------------

export function getAbortController(key: string): AbortController | null {
  return runners.get(key)?.abortController ?? null
}

export function setAbortController(key: string, controller: AbortController | null): void {
  ensureEntry(key).abortController = controller
}

export function getLastCachePoint(key: string): number | undefined {
  return runners.get(key)?.lastCachePoint
}

export function setLastCachePoint(key: string, index: number | undefined): void {
  ensureEntry(key).lastCachePoint = index
}

export function getLastAssistantMessageId(key: string): string | null {
  return runners.get(key)?.lastAssistantMessageId ?? null
}

export function setLastAssistantMessageId(key: string, id: string | null): void {
  ensureEntry(key).lastAssistantMessageId = id
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/** Abort a session's in-flight request and mark it idle. */
export function abortRun(key: string): void {
  const entry = runners.get(key)
  if (!entry) return
  if (entry.abortController) {
    entry.abortController.abort()
    entry.abortController = null
  }
  if (entry.state.loading || entry.state.executingTools.size > 0) {
    commit(entry, { ...entry.state, loading: false, executingTools: new Set() })
  }
}

/**
 * Drop an idle session's entry so switching between many chats doesn't retain every
 * message array. Running sessions are kept.
 */
export function releaseRunner(key: string): void {
  const entry = runners.get(key)
  if (!entry || entry.state.loading || entry.listeners.size > 0) return
  runners.delete(key)
  refreshRunningSnapshot()
}

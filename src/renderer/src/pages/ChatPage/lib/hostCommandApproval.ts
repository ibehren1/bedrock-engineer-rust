/**
 * Approval broker for host-targeted commands.
 *
 * When the Docker sandbox is enabled for an agent, commands run in the container by
 * default and anything is permitted there. Reaching the user's own machine requires
 * `target: "host"`, and that needs the user's consent — so the chat loop parks the
 * request here and a modal in ChatPage answers it.
 *
 * The gate lives in the renderer rather than in the preload tool for two reasons: only
 * the renderer can show UI, and the background-agent path (main → preload) bypasses this
 * module entirely, which is the intended behavior for unattended tasks.
 */

export type HostApprovalDecision = 'once' | 'chat' | 'deny'

export interface HostApprovalRequest {
  id: number
  sessionId?: string
  command: string
  cwd: string
}

type Listener = () => void

let nextId = 1
let pending: (HostApprovalRequest & { resolve: (decision: HostApprovalDecision) => void }) | null =
  null

/** Sessions where the user chose "allow for this chat". Deliberately not persisted. */
const sessionApprovals = new Set<string>()

const listeners = new Set<Listener>()

const notify = () => listeners.forEach((listener) => listener())

export const subscribeHostApproval = (listener: Listener): (() => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export const getPendingHostApproval = (): HostApprovalRequest | null =>
  pending
    ? { id: pending.id, sessionId: pending.sessionId, command: pending.command, cwd: pending.cwd }
    : null

/**
 * Ask the user whether a host command may run. Resolves immediately when the session
 * already carries a standing approval.
 *
 * A second concurrent request is denied rather than queued: the modal can only show one
 * command, and silently holding a tool call open invites confusion about what was
 * approved.
 */
export const requestHostApproval = (params: {
  sessionId?: string
  command: string
  cwd: string
}): Promise<HostApprovalDecision> => {
  if (params.sessionId && sessionApprovals.has(params.sessionId)) {
    return Promise.resolve('chat')
  }

  if (pending) {
    return Promise.resolve('deny')
  }

  return new Promise<HostApprovalDecision>((resolve) => {
    pending = {
      id: nextId++,
      sessionId: params.sessionId,
      command: params.command,
      cwd: params.cwd,
      resolve
    }
    notify()
  })
}

export const resolveHostApproval = (decision: HostApprovalDecision): void => {
  if (!pending) return

  const current = pending
  pending = null

  if (decision === 'chat' && current.sessionId) {
    sessionApprovals.add(current.sessionId)
  }

  notify()
  current.resolve(decision)
}

/**
 * Drop a session's standing approval. Called when the chat is left or deleted, so
 * "allow for this chat" does not outlive the chat it was granted in.
 */
export const clearHostApproval = (sessionId: string): void => {
  sessionApprovals.delete(sessionId)
}

export const hasSessionHostApproval = (sessionId: string): boolean =>
  sessionApprovals.has(sessionId)

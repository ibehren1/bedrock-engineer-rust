import { useCallback, useEffect, useState } from 'react'

/**
 * Which chats in the sidebar have a Docker sandbox, and which have attached files.
 *
 * Both are one call for the whole list rather than one per row: the sandbox list is already
 * a single query, and the attachments side takes the ids and answers in bulk for the same
 * reason. Refreshed when the set of chats changes, and on a slow timer so a sandbox created
 * by the agent mid-conversation shows up without a reload.
 */
export const useSessionResources = (sessionIds: string[], pollMs = 15_000) => {
  const [withSandbox, setWithSandbox] = useState<Set<string>>(new Set())
  const [withAttachments, setWithAttachments] = useState<Set<string>>(new Set())

  // Depend on a joined key rather than the array, so the effect does not re-run on every
  // render just because the caller built a new array with the same contents. The ids are
  // read back out of the key to keep the dependency honest.
  const key = sessionIds.join(',')

  const refresh = useCallback(async () => {
    const ids = key ? key.split(',') : []
    if (ids.length === 0) {
      setWithSandbox(new Set())
      setWithAttachments(new Set())
      return
    }

    const [sandboxes, attachments] = await Promise.all([
      window.api.dockerSandbox.list().catch(() => ({ sessionIds: [] as string[] })),
      window.api.chatAttachments.withFiles(ids).catch(() => ({ sessionIds: [] as string[] }))
    ])

    // The sandbox list covers every sandbox on disk, including chats the sidebar filters
    // out, so narrow it to what is actually on screen.
    const visible = new Set(ids)
    setWithSandbox(new Set((sandboxes.sessionIds ?? []).filter((id) => visible.has(id))))
    setWithAttachments(new Set(attachments.sessionIds ?? []))
  }, [key])

  useEffect(() => {
    void refresh()
    const timer = setInterval(() => void refresh(), pollMs)
    return () => clearInterval(timer)
  }, [refresh, pollMs])

  return { withSandbox, withAttachments, refresh }
}

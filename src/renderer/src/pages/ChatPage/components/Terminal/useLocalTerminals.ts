import { useCallback, useEffect, useState } from 'react'
import type { LocalTerminalInfo } from '@/types/bridge/api'

/**
 * The chat's shells on the user's own machine (the Terminal tab). They belong to the chat and
 * outlive the page: switching chats or leaving Chat only drops the view, and coming back lists
 * them again from the backend. They end when closed, when the chat is deleted, or on quit.
 */
export const useLocalTerminals = (sessionId: string | undefined) => {
  const [terminals, setTerminals] = useState<LocalTerminalInfo[]>([])
  const [selectedId, setSelectedId] = useState<string>()

  useEffect(() => {
    setTerminals([])
    setSelectedId(undefined)
    if (!sessionId) return
    let cancelled = false
    window.api.localTerminal
      .list(sessionId)
      .then((list) => {
        if (cancelled) return
        setTerminals(list)
        setSelectedId(list[list.length - 1]?.terminalId)
      })
      .catch((error) => console.warn('[LocalTerminal] could not list terminals', error))
    return () => {
      cancelled = true
    }
  }, [sessionId])

  /** Start a new shell and select it. The view attaches to it once it has a size. */
  const openTerminal = useCallback(async (): Promise<void> => {
    if (!sessionId) return
    const opened = await window.api.localTerminal.open(sessionId, 80, 24)
    const info: LocalTerminalInfo = {
      terminalId: opened.terminalId,
      channel: opened.channel,
      shell: opened.shell,
      cwd: opened.cwd,
      exited: false,
      exitCode: null,
      createdAt: Date.now()
    }
    setTerminals((list) => [...list, info])
    setSelectedId(info.terminalId)
  }, [sessionId])

  // The panel falls back to the last shell when the selected one is gone.
  const closeTerminal = useCallback((terminalId: string) => {
    void window.api.localTerminal.close(terminalId)
    setTerminals((list) => list.filter((item) => item.terminalId !== terminalId))
  }, [])

  const markExited = useCallback((terminalId: string, exitCode: number | null) => {
    setTerminals((list) =>
      list.map((item) =>
        item.terminalId === terminalId ? { ...item, exited: true, exitCode } : item
      )
    )
  }, [])

  return { terminals, selectedId, setSelectedId, openTerminal, closeTerminal, markExited }
}

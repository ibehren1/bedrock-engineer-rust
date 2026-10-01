import React, { useEffect, useState } from 'react'
import { FiLoader } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import { FiMoreHorizontal, FiEdit2, FiTrash2, FiZap, FiCheckSquare, FiX } from 'react-icons/fi'
import { RiArchiveStackLine } from 'react-icons/ri'
import { FaDocker, FaPaperclip } from 'react-icons/fa'
import { SessionMetadata } from '@/types/chat/history'
import { useChatHistory } from '@renderer/contexts/ChatHistoryContext'
import { generateSessionTitle } from '../../utils/titleGenerator'
import { useLightProcessingModel } from '@renderer/lib/modelSelection'
import { useRunningSessions } from '../../runners/useRunningSessions'
import { ConfirmDeleteChatModal, DeleteChatRequest } from '../../modals/ConfirmDeleteChatModal'
import { useSessionResources } from './useSessionResources'

interface ChatHistoryProps {
  onSessionSelect: (sessionId: string) => void
  currentSessionId?: string
}

export const ChatHistory: React.FC<ChatHistoryProps> = ({ onSessionSelect, currentSessionId }) => {
  const [editingSessionId, setEditingSessionId] = useState<string>()
  const [editTitle, setEditTitle] = useState('')
  const [menuOpenSessionId, setMenuOpenSessionId] = useState<string>()
  const [isGlobalMenuOpen, setIsGlobalMenuOpen] = useState(false)
  const [isComposing, setIsComposing] = useState(false)
  const [isGenerating, setIsGenerating] = useState(false)
  const [selectionMode, setSelectionMode] = useState(false)
  const [selectedSessionIds, setSelectedSessionIds] = useState<Set<string>>(new Set())
  const [deleteRequest, setDeleteRequest] = useState<DeleteChatRequest | null>(null)

  const { getLightModelId } = useLightProcessingModel()
  const { t } = useTranslation()
  // A turn keeps running after you switch away from its chat, so mark the ones still working.
  const runningSessionIds = useRunningSessions()

  // ChatHistoryContext から sessions と操作関数を取得
  const {
    sessions,
    getSession,
    updateSessionTitle,
    deleteSession,
    deleteSessions,
    deleteAllSessions,
    getSessionsWithSandbox
  } = useChatHistory()

  // Sandbox / attachment indicators for the rows below.
  const { withSandbox: sessionsWithSandbox, withAttachments: sessionsWithAttachments } =
    useSessionResources(sessions.map((session) => session.id))

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement
      if (editingSessionId && !target.closest('.editing-input')) {
        setEditingSessionId(undefined)
      }
      if (!target.closest('.global-menu') && !target.closest('.global-menu-button')) {
        setIsGlobalMenuOpen(false)
      }
      setMenuOpenSessionId(undefined)
    }
    document.addEventListener('click', handleClickOutside)
    return () => {
      document.removeEventListener('click', handleClickOutside)
    }
  }, [editingSessionId])

  const handleSessionClick = async (sessionId: string) => {
    if (selectionMode) {
      toggleSessionSelection(sessionId)
      return
    }
    onSessionSelect(sessionId)
  }

  const toggleSessionSelection = (sessionId: string) => {
    setSelectedSessionIds((prev) => {
      const next = new Set(prev)
      if (next.has(sessionId)) {
        next.delete(sessionId)
      } else {
        next.add(sessionId)
      }
      return next
    })
  }

  const enterSelectionMode = () => {
    setSelectionMode(true)
    setSelectedSessionIds(new Set())
    setIsGlobalMenuOpen(false)
    setMenuOpenSessionId(undefined)
  }

  const exitSelectionMode = () => {
    setSelectionMode(false)
    setSelectedSessionIds(new Set())
  }

  const allSelected = sessions.length > 0 && selectedSessionIds.size === sessions.length

  const toggleSelectAll = () => {
    setSelectedSessionIds((prev) =>
      prev.size === sessions.length ? new Set() : new Set(sessions.map((s) => s.id))
    )
  }

  // Open the confirm dialog, first checking whether any target chat owns a Docker
  // sandbox so the extra "also delete sandbox data" choice is only offered when it means
  // something.
  const openDeleteDialog = async (
    scope: DeleteChatRequest['scope'],
    sessionIds: string[]
  ): Promise<void> => {
    const withSandbox = await getSessionsWithSandbox(
      scope === 'all' ? sessions.map((session) => session.id) : sessionIds
    )
    setDeleteRequest({ scope, sessionIds, hasSandbox: withSandbox.length > 0 })
  }

  const handleDeleteSelected = () => {
    if (selectedSessionIds.size === 0) return
    void openDeleteDialog('selected', Array.from(selectedSessionIds))
  }

  const handleDeleteSession = async (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation()
    setMenuOpenSessionId(undefined)
    await openDeleteDialog('single', [sessionId])
  }

  const handleDeleteAllSessions = async (e: React.MouseEvent) => {
    e.stopPropagation()
    setIsGlobalMenuOpen(false)
    await openDeleteDialog('all', [])
  }

  const handleConfirmDelete = async (deleteSandboxData: boolean) => {
    const request = deleteRequest
    setDeleteRequest(null)
    if (!request) return

    if (request.scope === 'all') {
      await deleteAllSessions(deleteSandboxData)
      return
    }
    if (request.scope === 'selected') {
      await deleteSessions(request.sessionIds, deleteSandboxData)
      exitSelectionMode()
      return
    }
    await deleteSession(request.sessionIds[0], deleteSandboxData)
  }

  const startEditing = (sessionId: string, title: string, e: React.MouseEvent) => {
    e.stopPropagation()
    setEditingSessionId(sessionId)
    setEditTitle(title)
    setMenuOpenSessionId(undefined)
  }

  const saveTitle = (sessionId: string, e: React.MouseEvent | React.KeyboardEvent) => {
    e.stopPropagation()
    if (editTitle.trim()) {
      updateSessionTitle(sessionId, editTitle.trim())
    }
    setEditingSessionId(undefined)
  }

  const generateAITitle = async (session: SessionMetadata, e: React.MouseEvent) => {
    e.stopPropagation()
    setIsGenerating(true)

    try {
      // セッションの詳細を取得
      const sessionDetails = getSession(session.id)
      if (!sessionDetails) {
        throw new Error('Session not found')
      }

      // 軽量処理用モデルIDを取得
      const lightModelId = getLightModelId()

      // 軽量モデルでタイトルを生成
      const newTitle = await generateSessionTitle(sessionDetails, lightModelId, t)

      if (newTitle) {
        // タイトルを更新
        updateSessionTitle(session.id, newTitle)
        setMenuOpenSessionId(undefined)
      }
    } catch (error) {
      console.error('Failed to generate AI title:', error)
      // エラーメッセージは generateSessionTitle 内で既に表示されるため不要
    } finally {
      setIsGenerating(false)
    }
  }

  const generateAITitleForAllSession = async (e: React.MouseEvent) => {
    try {
      setIsGenerating(true)
      for (const session of sessions) {
        // タイトルが 'Chat' で始まるセッションのみ対象とする
        if (session.title.startsWith('Chat')) {
          await generateAITitle(session, e)
        }
      }
      setIsGlobalMenuOpen(false)
    } finally {
      setIsGenerating(false)
    }
  }

  const toggleMenu = (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation()
    setMenuOpenSessionId(menuOpenSessionId === sessionId ? undefined : sessionId)
  }

  const toggleGlobalMenu = (e: React.MouseEvent) => {
    e.stopPropagation()
    setIsGlobalMenuOpen(!isGlobalMenuOpen)
  }

  const handleCompositionStart = () => {
    setIsComposing(true)
  }

  const handleCompositionEnd = () => {
    setIsComposing(false)
  }

  const handleKeyDown = (e: React.KeyboardEvent, sessionId: string) => {
    if (isComposing) return

    if (e.key === 'Enter') {
      saveTitle(sessionId, e)
    } else if (e.key === 'Escape') {
      setEditingSessionId(undefined)
    }
  }

  if (sessions.length === 0) {
    return <div className="p-2.5 text-center text-ink-muted">{t('No chat history')}</div>
  }

  const menuButtonClasses =
    'text-ink-faint hover:text-ink-muted p-1 rounded-control hover:bg-raised h-8 w-8 flex items-center justify-center'

  const formatDate = (timestamp: number) => {
    return new Date(timestamp).toLocaleString()
  }

  return (
    <div className="chat-history p-3">
      <h2
        className="text-sm font-semibold mb-4 text-ink flex items-center justify-between hover:cursor-pointer hover:text-ink-muted"
        onClick={toggleGlobalMenu}
      >
        <div className="flex items-center">
          <RiArchiveStackLine className="inline-block mr-2 w-4 h-4" />
          {t('Chat History')}
        </div>
        <div className="relative">
          {isGlobalMenuOpen && (
            <div
              className="global-menu absolute right-0 mt-1 w-48 bg-surface rounded-control shadow-lg border border-subtle z-10"
              onClick={(e) => e.stopPropagation()}
            >
              <div className="py-1">
                <button
                  onClick={generateAITitleForAllSession}
                  disabled={isGenerating}
                  className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
                >
                  {isGenerating ? (
                    <FiLoader className="animate-spin w-4 h-4" />
                  ) : (
                    <FiZap className="w-4 h-4" />
                  )}
                  {t('Generate All Titles')}
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    enterSelectionMode()
                  }}
                  className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2 border-t border-faint"
                >
                  <FiCheckSquare className="w-4 h-4" />
                  {t('Select')}
                </button>
                <button
                  onClick={handleDeleteAllSessions}
                  className="w-full text-left px-2.5 py-1 text-sm text-danger hover:bg-raised flex items-center gap-2 border-t border-faint"
                >
                  <FiTrash2 className="w-4 h-4" />
                  {t('Delete All')}
                </button>
              </div>
            </div>
          )}
        </div>
      </h2>

      {/* Selection action bar */}
      {selectionMode && (
        <div className="flex items-center justify-between mb-3 px-1">
          <div className="flex items-center gap-3">
            <span className="text-xs text-ink-muted">
              {t('{{count}} selected', { count: selectedSessionIds.size })}
            </span>
            <button onClick={toggleSelectAll} className="text-xs text-accent hover:text-accent">
              {allSelected ? t('Clear selection') : t('Select all')}
            </button>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={handleDeleteSelected}
              disabled={selectedSessionIds.size === 0}
              title={t('Delete selected')}
              className="p-1.5 rounded-control text-danger hover:bg-raised disabled:opacity-40 disabled:cursor-not-allowed flex items-center gap-1"
            >
              <FiTrash2 className="w-4 h-4" />
            </button>
            <button
              onClick={exitSelectionMode}
              title={t('Cancel selection')}
              className="p-1.5 rounded-control text-ink-muted hover:bg-raised"
            >
              <FiX className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      <div className="session-list space-y-2">
        {sessions.map((session) => (
          <div
            key={session.id}
            onClick={() => handleSessionClick(session.id)}
            className={`session-item p-3 rounded-container cursor-pointer hover:bg-sunken transition-colors duration-200
              ${
                selectionMode && selectedSessionIds.has(session.id)
                  ? 'bg-accent-tint ring-1 ring-accent'
                  : currentSessionId === session.id
                    ? 'bg-raised'
                    : ''
              }`}
          >
            <div className="flex justify-between items-center min-h-[32px] gap-2">
              {selectionMode && editingSessionId !== session.id && (
                <input
                  type="checkbox"
                  checked={selectedSessionIds.has(session.id)}
                  onChange={() => toggleSessionSelection(session.id)}
                  onClick={(e) => e.stopPropagation()}
                  className="h-4 w-4 flex-shrink-0 text-accent focus:ring-accent border-strong rounded-control cursor-pointer"
                />
              )}
              {editingSessionId === session.id ? (
                <div
                  className="flex items-center w-full editing-input"
                  onClick={(e) => e.stopPropagation()}
                >
                  <input
                    type="text"
                    value={editTitle}
                    onChange={(e) => setEditTitle(e.target.value)}
                    className="flex-1 px-2 py-1 text-sm border rounded-control bg-surface border-subtle min-w-0"
                    autoFocus
                    onKeyDown={(e) => handleKeyDown(e, session.id)}
                    onCompositionStart={handleCompositionStart}
                    onCompositionEnd={handleCompositionEnd}
                  />
                </div>
              ) : (
                <div className="flex items-center justify-between w-full">
                  <div className="relative flex-1 min-w-0 pr-2">
                    <div className="flex items-center gap-1.5 min-w-0">
                      <h3 className="font-medium text-ink text-sm truncate" title={session.title}>
                        {session.title}
                      </h3>
                      {/* Both icons inherit the row's text colour. The whale is Docker blue
                          everywhere it is an action, but here it is one of several bits of
                          metadata on a dense row, so it stays quiet like the paperclip. */}
                      {sessionsWithAttachments.has(session.id) && (
                        <FaPaperclip
                          className="size-2.5 shrink-0 text-ink-faint"
                          title={t('history.hasAttachments')}
                          aria-label={t('history.hasAttachments')}
                        />
                      )}
                      {sessionsWithSandbox.has(session.id) && (
                        <FaDocker
                          className="size-3 shrink-0 text-ink-faint"
                          title={t('history.hasSandbox')}
                          aria-label={t('history.hasSandbox')}
                        />
                      )}
                    </div>
                    {runningSessionIds.includes(session.id) ? (
                      <p className="text-xs text-accent whitespace-nowrap flex items-center gap-1.5">
                        <span className="relative flex h-1.5 w-1.5 flex-shrink-0">
                          <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-accent-tint opacity-75" />
                          <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-accent-tint" />
                        </span>
                        {t('Still responding')}
                      </p>
                    ) : (
                      <p className="text-xs text-ink-muted whitespace-nowrap">
                        {formatDate(session.updatedAt)} · {session.messageCount} messages
                      </p>
                    )}
                  </div>
                  <div className={`relative flex-shrink-0 ${selectionMode ? 'hidden' : ''}`}>
                    <button
                      onClick={(e) => toggleMenu(session.id, e)}
                      className={menuButtonClasses}
                    >
                      <FiMoreHorizontal className="w-4 h-4" />
                    </button>
                    {menuOpenSessionId === session.id && (
                      <div
                        className="absolute right-0 mt-1 w-48 bg-surface rounded-control shadow-lg border border-subtle z-10"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <div className="py-1">
                          <button
                            onClick={(e) => startEditing(session.id, session.title, e)}
                            className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
                          >
                            <FiEdit2 className="w-4 h-4" />
                            {t('Edit title')}
                          </button>
                          <button
                            onClick={(e) => generateAITitle(session, e)}
                            disabled={isGenerating}
                            className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2 border-t border-faint disabled:opacity-50 disabled:cursor-not-allowed"
                          >
                            {isGenerating ? (
                              <FiLoader className="animate-spin w-4 h-4" />
                            ) : (
                              <FiZap className="w-4 h-4" />
                            )}
                            {t('Generate title')}
                          </button>
                          <button
                            onClick={(e) => handleDeleteSession(session.id, e)}
                            className="w-full text-left px-2.5 py-1 text-sm text-danger hover:bg-raised flex items-center gap-2 border-t border-faint"
                          >
                            <FiTrash2 className="w-4 h-4" />
                            {t('Delete')}
                          </button>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>

      <ConfirmDeleteChatModal
        request={deleteRequest}
        onCancel={() => setDeleteRequest(null)}
        onConfirm={handleConfirmDelete}
      />
    </div>
  )
}

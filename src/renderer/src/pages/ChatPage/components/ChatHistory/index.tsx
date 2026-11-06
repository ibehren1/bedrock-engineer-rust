import { useSessionCosts } from '@renderer/hooks/useSessionCosts'
import { formatUsd } from '@renderer/pages/AccountingPage/accountingUtils'
import React, { useEffect, useMemo, useState } from 'react'
import { FiLoader } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import {
  FiMoreHorizontal,
  FiEdit2,
  FiTrash2,
  FiZap,
  FiCheckSquare,
  FiX,
  FiFolder,
  FiFolderPlus,
  FiFolderMinus,
  FiChevronDown,
  FiChevronRight,
  FiArrowUp,
  FiArrowDown
} from 'react-icons/fi'
import { RiArchiveStackLine } from 'react-icons/ri'
import { FaDocker, FaPaperclip } from 'react-icons/fa'
import { SessionMetadata } from '@/types/chat/history'
import { useChatHistory } from '@renderer/contexts/ChatHistoryContext'
import { generateSessionTitle } from '../../utils/titleGenerator'
import { useLightProcessingModel } from '@renderer/lib/modelSelection'
import { useRunningSessions } from '../../runners/useRunningSessions'
import { ConfirmDeleteChatModal, DeleteChatRequest } from '../../modals/ConfirmDeleteChatModal'
import { useSessionResources } from './useSessionResources'
import {
  CHAT_DRAG_TYPE,
  FOLDER_DRAG_TYPE,
  draggedChatIds,
  groupSessionsByFolder,
  isChatDrag,
  isFolderDrag,
  moveFolder,
  shiftFolder
} from './groupSessions'

/** localStorage key holding the ids of folded folders. */
const FOLDED_FOLDERS_KEY = 'chatHistory.foldedFolders'

function readFoldedFolders(): Set<string> {
  try {
    const ids: unknown = JSON.parse(localStorage.getItem(FOLDED_FOLDERS_KEY) || '[]')
    return new Set(Array.isArray(ids) ? ids.filter((id) => typeof id === 'string') : [])
  } catch {
    return new Set()
  }
}

/** Drop target key for the chats in no folder (and the heading). */
const LOOSE = 'loose'

interface ChatHistoryProps {
  onSessionSelect: (sessionId: string) => void
  currentSessionId?: string
  /** A control at the right of the heading (the sidebar's collapse button). */
  headerAction?: React.ReactNode
}

export const ChatHistory: React.FC<ChatHistoryProps> = ({
  onSessionSelect,
  currentSessionId,
  headerAction
}) => {
  const [editingSessionId, setEditingSessionId] = useState<string>()
  const [editTitle, setEditTitle] = useState('')
  const [menuOpenSessionId, setMenuOpenSessionId] = useState<string>()
  const [isGlobalMenuOpen, setIsGlobalMenuOpen] = useState(false)
  const [isComposing, setIsComposing] = useState(false)
  const [isGenerating, setIsGenerating] = useState(false)
  const [selectionMode, setSelectionMode] = useState(false)
  const [selectedSessionIds, setSelectedSessionIds] = useState<Set<string>>(new Set())
  const [deleteRequest, setDeleteRequest] = useState<DeleteChatRequest | null>(null)
  const [editingFolderId, setEditingFolderId] = useState<string>()
  const [folderName, setFolderName] = useState('')
  const [folderMenuOpenId, setFolderMenuOpenId] = useState<string>()
  const [foldedFolders, setFoldedFolders] = useState<Set<string>>(readFoldedFolders)
  // A chat is being dragged, and the folder (or LOOSE) under the pointer.
  const [dragging, setDragging] = useState(false)
  const [dropTarget, setDropTarget] = useState<string | null>(null)
  // A folder header is being dragged, and where it would land.
  const [draggedFolderId, setDraggedFolderId] = useState<string | null>(null)
  const [folderDrop, setFolderDrop] = useState<{
    id: string
    position: 'before' | 'after'
  } | null>(null)

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
    getSessionsWithSandbox,
    folders,
    createFolder,
    renameFolder,
    reorderFolders,
    deleteFolder,
    moveSessionsToFolder
  } = useChatHistory()
  const grouped = useMemo(() => groupSessionsByFolder(sessions, folders), [sessions, folders])

  // Sandbox / attachment indicators for the rows below.
  const { withSandbox: sessionsWithSandbox, withAttachments: sessionsWithAttachments } =
    useSessionResources(sessions.map((session) => session.id))
  // Estimated cost of each chat, from the usage ledger.
  const costWindows = useMemo(() => sessions.map((s) => ({ sessionId: s.id })), [sessions])
  const sessionCosts = useSessionCosts(costWindows)
  const costOf = (index: number) => sessionCosts[index] ?? 0

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement
      if (editingSessionId && !target.closest('.editing-input')) {
        setEditingSessionId(undefined)
      }
      if (editingFolderId && !target.closest('.folder-editing-input')) {
        setEditingFolderId(undefined)
      }
      setFolderMenuOpenId(undefined)
      if (!target.closest('.global-menu') && !target.closest('.global-menu-button')) {
        setIsGlobalMenuOpen(false)
      }
      setMenuOpenSessionId(undefined)
    }
    document.addEventListener('click', handleClickOutside)
    return () => {
      document.removeEventListener('click', handleClickOutside)
    }
  }, [editingSessionId, editingFolderId])

  const toggleFolded = (folderId: string) => {
    setFoldedFolders((prev) => {
      const next = new Set(prev)
      if (next.has(folderId)) next.delete(folderId)
      else next.add(folderId)
      try {
        localStorage.setItem(FOLDED_FOLDERS_KEY, JSON.stringify([...next]))
      } catch {
        // Only the folded state is lost.
      }
      return next
    })
  }

  const handleNewFolder = async (e: React.MouseEvent) => {
    e.stopPropagation()
    setIsGlobalMenuOpen(false)
    const name = t('history.defaultFolderName')
    try {
      const id = await createFolder(name)
      setEditingFolderId(id)
      setFolderName(name)
    } catch (error) {
      console.error('Failed to create folder:', error)
    }
  }

  const startEditingFolder = (folderId: string, name: string, e: React.MouseEvent) => {
    e.stopPropagation()
    setFolderMenuOpenId(undefined)
    setEditingFolderId(folderId)
    setFolderName(name)
  }

  const saveFolderName = (folderId: string) => {
    if (folderName.trim()) void renameFolder(folderId, folderName.trim())
    setEditingFolderId(undefined)
  }

  const handleDeleteFolder = (folderId: string, name: string, e: React.MouseEvent) => {
    e.stopPropagation()
    setFolderMenuOpenId(undefined)
    if (window.confirm(t('history.deleteFolderConfirm', { name }))) void deleteFolder(folderId)
  }

  const moveTo = (sessionId: string, folderId: string | null, e: React.MouseEvent) => {
    e.stopPropagation()
    setMenuOpenSessionId(undefined)
    void moveSessionsToFolder([sessionId], folderId)
  }

  // Dragging a row carries its id, or every selected id when it is one of the selection.
  const handleDragStart = (sessionId: string, e: React.DragEvent) => {
    const ids =
      selectionMode && selectedSessionIds.has(sessionId)
        ? Array.from(selectedSessionIds)
        : [sessionId]
    e.dataTransfer.setData(CHAT_DRAG_TYPE, JSON.stringify(ids))
    e.dataTransfer.effectAllowed = 'move'
    setMenuOpenSessionId(undefined)
    setDragging(true)
  }

  const handleDragEnd = () => {
    setDragging(false)
    setDropTarget(null)
  }

  const folderIds = () => folders.map((f) => f.id)

  // Dragging a folder header reorders the folders.
  const handleFolderDragStart = (folderId: string, e: React.DragEvent) => {
    e.dataTransfer.setData(FOLDER_DRAG_TYPE, folderId)
    e.dataTransfer.effectAllowed = 'move'
    setFolderMenuOpenId(undefined)
    setDraggedFolderId(folderId)
  }

  const handleFolderDragEnd = () => {
    setDraggedFolderId(null)
    setFolderDrop(null)
  }

  const shiftFolderBy = (folderId: string, by: -1 | 1, e: React.MouseEvent) => {
    e.stopPropagation()
    setFolderMenuOpenId(undefined)
    const ids = folderIds()
    const next = shiftFolder(ids, folderId, by)
    if (next !== ids) void reorderFolders(next)
  }

  /** Drop handlers for a folder (`folderId`) or, with `null`, the chats in no folder. */
  const dropProps = (folderId: string | null) => {
    const key = folderId ?? LOOSE
    return {
      onDragOver: (e: React.DragEvent) => {
        if (folderId && isFolderDrag(e)) {
          e.preventDefault()
          e.dataTransfer.dropEffect = 'move'
          const rect = e.currentTarget.getBoundingClientRect()
          const position = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after'
          if (folderDrop?.id !== folderId || folderDrop.position !== position) {
            setFolderDrop({ id: folderId, position })
          }
          return
        }
        if (!isChatDrag(e)) return
        e.preventDefault()
        e.dataTransfer.dropEffect = 'move'
        if (dropTarget !== key) setDropTarget(key)
      },
      onDragLeave: (e: React.DragEvent) => {
        if (e.currentTarget.contains(e.relatedTarget as Node | null)) return
        setDropTarget((current) => (current === key ? null : current))
        setFolderDrop((current) => (current?.id === folderId ? null : current))
      },
      onDrop: (e: React.DragEvent) => {
        if (folderId && isFolderDrag(e)) {
          e.preventDefault()
          e.stopPropagation()
          const moved = e.dataTransfer.getData(FOLDER_DRAG_TYPE)
          const position = folderDrop?.id === folderId ? folderDrop.position : 'before'
          handleFolderDragEnd()
          const ids = folderIds()
          const next = moveFolder(ids, moved, folderId, position)
          if (next !== ids) void reorderFolders(next)
          return
        }
        if (!isChatDrag(e)) return
        e.preventDefault()
        e.stopPropagation()
        const ids = draggedChatIds(e.dataTransfer)
        handleDragEnd()
        if (ids.length > 0) void moveSessionsToFolder(ids, folderId)
      }
    }
  }

  const dropHighlight = (key: string) =>
    dropTarget === key ? 'ring-1 ring-accent bg-accent-tint' : ''

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

  if (sessions.length === 0 && folders.length === 0) {
    return (
      <div className="p-3">
        {headerAction && <div className="flex justify-end">{headerAction}</div>}
        <div className="p-2.5 text-center text-ink-muted">{t('No chat history')}</div>
      </div>
    )
  }

  const menuButtonClasses =
    'text-ink-faint hover:text-ink-muted p-1 rounded-control hover:bg-raised h-8 w-8 flex items-center justify-center'

  const formatDate = (timestamp: number) => {
    return new Date(timestamp).toLocaleString()
  }

  const renderSession = (session: SessionMetadata) => (
    <div
      key={session.id}
      onClick={() => handleSessionClick(session.id)}
      draggable={editingSessionId !== session.id}
      onDragStart={(e) => handleDragStart(session.id, e)}
      onDragEnd={handleDragEnd}
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
            className="h-4 w-4 shrink-0 text-accent focus:ring-accent border-strong rounded-control cursor-pointer"
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
                  <span className="relative flex h-1.5 w-1.5 shrink-0">
                    <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-accent-tint opacity-75" />
                    <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-accent-tint" />
                  </span>
                  {t('Still responding')}
                </p>
              ) : (
                <p className="text-xs text-ink-muted whitespace-nowrap">
                  {formatDate(session.updatedAt)} · {session.messageCount} messages
                  {costOf(sessions.indexOf(session)) > 0 &&
                    ` · ${formatUsd(costOf(sessions.indexOf(session)))}`}
                </p>
              )}
            </div>
            <div className={`relative shrink-0 ${selectionMode ? 'hidden' : ''}`}>
              <button onClick={(e) => toggleMenu(session.id, e)} className={menuButtonClasses}>
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
                    {folders.length > 0 && (
                      <div className="border-t border-faint">
                        <div className="px-2.5 pt-1 text-xs text-ink-muted">
                          {t('history.moveToFolder')}
                        </div>
                        {folders
                          .filter((folder) => folder.id !== session.folderId)
                          .map((folder) => (
                            <button
                              key={folder.id}
                              onClick={(e) => moveTo(session.id, folder.id, e)}
                              className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
                            >
                              <FiFolder className="w-4 h-4 shrink-0" />
                              <span className="truncate">{folder.name}</span>
                            </button>
                          ))}
                        {session.folderId && grouped.byFolder.has(session.folderId) && (
                          <button
                            onClick={(e) => moveTo(session.id, null, e)}
                            className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
                          >
                            <FiFolderMinus className="w-4 h-4 shrink-0" />
                            {t('history.noFolder')}
                          </button>
                        )}
                      </div>
                    )}
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
  )

  return (
    <div className="chat-history p-3">
      <h2
        className={`text-sm font-semibold mb-4 text-ink flex items-center justify-between hover:cursor-pointer hover:text-ink-muted rounded-control ${dropHighlight(LOOSE)}`}
        onClick={toggleGlobalMenu}
        {...dropProps(null)}
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
                  onClick={handleNewFolder}
                  className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2 border-b border-faint"
                >
                  <FiFolderPlus className="w-4 h-4" />
                  {t('history.newFolder')}
                </button>
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
          {headerAction && <span onClick={(e) => e.stopPropagation()}>{headerAction}</span>}
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

      {/* Folders, then the chats in no folder. Rows can be dragged onto a folder, or onto the
          loose list or the heading to take them out of their folder. */}
      {folders.length > 0 && (
        <div className="space-y-2 mb-2">
          {folders.map((folder, index) => {
            const items = grouped.byFolder.get(folder.id) ?? []
            const folded = foldedFolders.has(folder.id)
            const editing = editingFolderId === folder.id
            // The insertion line shows where a dragged folder would land.
            const insertAt =
              draggedFolderId && draggedFolderId !== folder.id && folderDrop?.id === folder.id
                ? folderDrop.position
                : null
            return (
              <div
                key={folder.id}
                className={`relative rounded-container ${dropHighlight(folder.id)} ${
                  draggedFolderId === folder.id ? 'opacity-50' : ''
                }`}
                {...dropProps(folder.id)}
              >
                {insertAt && (
                  <div
                    className={`pointer-events-none absolute left-0 right-0 h-0.5 bg-accent ${
                      insertAt === 'before' ? '-top-1' : '-bottom-1'
                    }`}
                  />
                )}
                <div
                  className="flex items-center gap-1.5 px-2 py-1.5 rounded-container cursor-pointer hover:bg-sunken"
                  onClick={() => !editing && toggleFolded(folder.id)}
                  draggable={!editing}
                  onDragStart={(e) => handleFolderDragStart(folder.id, e)}
                  onDragEnd={handleFolderDragEnd}
                >
                  {folded ? (
                    <FiChevronRight className="w-4 h-4 shrink-0 text-ink-muted" />
                  ) : (
                    <FiChevronDown className="w-4 h-4 shrink-0 text-ink-muted" />
                  )}
                  <FiFolder className="w-4 h-4 shrink-0 text-ink-muted" />
                  {editing ? (
                    <input
                      type="text"
                      value={folderName}
                      onChange={(e) => setFolderName(e.target.value)}
                      onClick={(e) => e.stopPropagation()}
                      onKeyDown={(e) => {
                        if (isComposing) return
                        if (e.key === 'Enter') saveFolderName(folder.id)
                        else if (e.key === 'Escape') setEditingFolderId(undefined)
                      }}
                      onCompositionStart={handleCompositionStart}
                      onCompositionEnd={handleCompositionEnd}
                      onFocus={(e) => e.target.select()}
                      autoFocus
                      className="folder-editing-input flex-1 px-2 py-0.5 text-sm border rounded-control bg-surface border-subtle min-w-0"
                    />
                  ) : (
                    <span
                      className="flex-1 min-w-0 text-sm font-medium text-ink truncate"
                      title={folder.name}
                    >
                      {folder.name}
                    </span>
                  )}
                  {!editing && (
                    <>
                      <span className="text-xs text-ink-muted whitespace-nowrap">
                        {t('history.chatCount', { count: items.length })}
                      </span>
                      <div className="relative shrink-0">
                        <button
                          onClick={(e) => {
                            e.stopPropagation()
                            setMenuOpenSessionId(undefined)
                            setFolderMenuOpenId(
                              folderMenuOpenId === folder.id ? undefined : folder.id
                            )
                          }}
                          className={menuButtonClasses}
                        >
                          <FiMoreHorizontal className="w-4 h-4" />
                        </button>
                        {folderMenuOpenId === folder.id && (
                          <div
                            className="absolute right-0 mt-1 w-48 bg-surface rounded-control shadow-lg border border-subtle z-10"
                            onClick={(e) => e.stopPropagation()}
                          >
                            <div className="py-1">
                              <button
                                onClick={(e) => startEditingFolder(folder.id, folder.name, e)}
                                className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
                              >
                                <FiEdit2 className="w-4 h-4" />
                                {t('history.renameFolder')}
                              </button>
                              {index > 0 && (
                                <button
                                  onClick={(e) => shiftFolderBy(folder.id, -1, e)}
                                  className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
                                >
                                  <FiArrowUp className="w-4 h-4" />
                                  {t('history.moveFolderUp')}
                                </button>
                              )}
                              {index < folders.length - 1 && (
                                <button
                                  onClick={(e) => shiftFolderBy(folder.id, 1, e)}
                                  className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
                                >
                                  <FiArrowDown className="w-4 h-4" />
                                  {t('history.moveFolderDown')}
                                </button>
                              )}
                              <button
                                onClick={(e) => handleDeleteFolder(folder.id, folder.name, e)}
                                className="w-full text-left px-2.5 py-1 text-sm text-danger hover:bg-raised flex items-center gap-2 border-t border-faint"
                              >
                                <FiTrash2 className="w-4 h-4" />
                                {t('history.deleteFolder')}
                              </button>
                            </div>
                          </div>
                        )}
                      </div>
                    </>
                  )}
                </div>
                {!folded && items.length > 0 && (
                  <div className="ml-4 pl-2 border-l border-subtle space-y-2 mt-1">
                    {items.map(renderSession)}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      )}

      <div
        className={`session-list space-y-2 rounded-container ${dropHighlight(LOOSE)} ${
          dragging && grouped.loose.length === 0
            ? 'min-h-12 border border-dashed border-strong grid place-items-center'
            : ''
        }`}
        {...dropProps(null)}
      >
        {grouped.loose.map(renderSession)}
        {dragging && grouped.loose.length === 0 && (
          <p className="text-xs text-ink-muted px-2">{t('history.dropToRemoveFromFolder')}</p>
        )}
      </div>

      <ConfirmDeleteChatModal
        request={deleteRequest}
        onCancel={() => setDeleteRequest(null)}
        onConfirm={handleConfirmDelete}
      />
    </div>
  )
}

import React, { useCallback, useState, useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { FaClock, FaCheckCircle, FaTimesCircle, FaSpinner, FaTimes, FaSync } from 'react-icons/fa'
import { FaListCheck } from 'react-icons/fa6'

// Import shared types from common location
import type { TodoList, TodoItemStatus } from '../../../../../types/tools'

// Signal emitted when the TODO list changes, used to flash the header icon
export interface TodoFlash {
  status: TodoItemStatus
  nonce: number
}

interface TodoModalProps {
  isOpen: boolean
  onClose: () => void
  todoList?: TodoList | null
  onRefresh?: () => void
  loading?: boolean
}

// Get status icon
const getStatusIcon = (status: string) => {
  switch (status) {
    case 'pending':
      return <FaClock className="text-warning" />
    case 'in_progress':
      return <FaSpinner className="text-accent animate-spin" />
    case 'completed':
      return <FaCheckCircle className="text-success" />
    case 'cancelled':
      return <FaTimesCircle className="text-danger" />
    default:
      return <FaClock className="text-ink-muted" />
  }
}

// Todo Floating Window Component
export const TodoModal: React.FC<TodoModalProps> = ({
  isOpen,
  onClose,
  todoList,
  onRefresh,
  loading
}) => {
  const { t } = useTranslation()

  if (!isOpen) return null

  return (
    <div className="fixed top-4 right-4 z-50 w-80 max-h-[48rem] overflow-hidden transform transition-all duration-200 ease-in-out opacity-100">
      <div className="bg-surface border border-subtle rounded-container shadow-lg">
        {/* Header */}
        <div className="flex items-center justify-between p-2.5 border-b border-subtle">
          <div className="flex items-center gap-3">
            <FaListCheck className="text-accent" size={16} />
            <span className="font-semibold text-ink">{t('TODO List')}</span>
          </div>
          <div className="flex items-center gap-2">
            {onRefresh && (
              <button
                onClick={onRefresh}
                disabled={loading}
                className="p-1 rounded-full hover:bg-raised transition-colors disabled:opacity-50"
                title={t('common.refresh')}
              >
                <FaSync className={`text-ink-muted ${loading ? 'animate-spin' : ''}`} size={12} />
              </button>
            )}
            <button
              onClick={onClose}
              className="p-1 rounded-full hover:bg-raised transition-colors"
            >
              <FaTimes className="text-ink-muted" size={14} />
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="p-2.5 max-h-[30rem] overflow-y-auto">
          {!todoList || !todoList.items.length ? (
            // Empty state
            <div className="flex flex-col items-center justify-center py-8 text-center text-ink-muted">
              <FaListCheck className="text-3xl mb-2 text-ink-faint" />
              <h3 className="font-medium mb-1">{t('No TODO List Available')}</h3>
              <p className="text-xs">
                {t('Create a TODO list using the todoInit tool to see tasks here.')}
              </p>
            </div>
          ) : (
            // Simple task list
            <div className="space-y-2">
              {todoList.items.map((item) => (
                <div
                  key={item.id}
                  className="flex items-start gap-3 p-2 rounded-control hover:bg-surface-2 transition-colors"
                >
                  <div className="flex-shrink-0">{getStatusIcon(item.status)}</div>
                  <span className="flex-grow text-sm text-ink">{item.description}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

// Build a comparable signature of the list's item statuses
const buildStatusSignature = (list: TodoList | null): string =>
  list ? list.items.map((item) => `${item.id}:${item.status}`).join('|') : ''

// Pick the status to flash from the changed items (prefer the most advanced state)
const STATUS_PRIORITY: TodoItemStatus[] = ['completed', 'in_progress', 'cancelled', 'pending']
const pickFlashStatus = (prev: TodoList | null, next: TodoList | null): TodoItemStatus | null => {
  if (!next) return null
  const prevById = new Map((prev?.items ?? []).map((item) => [item.id, item.status]))
  const changed = next.items
    .filter((item) => prevById.get(item.id) !== item.status)
    .map((item) => item.status)
  if (changed.length === 0) return null
  return STATUS_PRIORITY.find((status) => changed.includes(status)) ?? changed[0]
}

// Custom hook for todo modal with real-time updates
export const useTodoModal = (
  messages?: any[],
  currentSessionId?: string,
  /** True while a response is streaming; the poll only runs then (or while open). */
  isStreaming?: boolean
) => {
  const [show, setShow] = useState(false)
  const [todoList, setTodoList] = useState<TodoList | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [flash, setFlash] = useState<TodoFlash | null>(null)

  // Track the last-seen list/signature/session to detect real changes.
  // Kept in refs so fetchTodoList stays stable and doesn't cause a poll loop.
  const prevListRef = useRef<TodoList | null>(null)
  const lastSignatureRef = useRef<string>('')
  const lastSessionRef = useRef<string | undefined>(undefined)
  const flashNonceRef = useRef(0)

  const fetchTodoList = useCallback(
    async (options?: { silent?: boolean }) => {
      try {
        if (!options?.silent) setLoading(true)
        setError(null)

        // Only fetch if session ID exists (no fallback behavior)
        if (!currentSessionId) {
          setTodoList(null)
          return
        }

        const data = await window.api.todo.getTodoList({ sessionId: currentSessionId })

        // Detect status changes and flash the header icon accordingly.
        // Suppress flashing on the first load of a session so switching
        // sessions doesn't trigger a spurious flash.
        const signature = buildStatusSignature(data)
        const isNewSession = lastSessionRef.current !== currentSessionId
        if (!isNewSession && signature !== lastSignatureRef.current) {
          const status = pickFlashStatus(prevListRef.current, data)
          if (status) {
            flashNonceRef.current += 1
            setFlash({ status, nonce: flashNonceRef.current })
          }
        }
        prevListRef.current = data
        lastSignatureRef.current = signature
        lastSessionRef.current = currentSessionId

        setTodoList(data)
      } catch (err) {
        console.error('Failed to fetch TODO list:', err)
        setError(err instanceof Error ? err.message : 'Failed to fetch TODO list')
        setTodoList(null)
      } finally {
        setLoading(false)
      }
    },
    [currentSessionId]
  )

  // Poll for TODO list changes while a response is streaming or the panel is
  // open. `todoUpdate` tool calls happen mid-stream inside an existing assistant
  // message, so messages.length is not a reliable signal — a short interval
  // reliably catches status changes so the header icon can flash. An idle chat
  // needs no poll at all: nothing can change the list, and each tick costs an
  // IPC round trip plus a file read in the main process.
  const hasMessages = !!messages && messages.length > 0
  const shouldPoll = hasMessages && (isStreaming || show)
  useEffect(() => {
    if (!shouldPoll) return undefined

    // Fetch immediately, then on a short interval (silent: no spinner flicker)
    fetchTodoList({ silent: true })
    const interval = setInterval(() => {
      fetchTodoList({ silent: true })
    }, 2000) // Refresh every 2 seconds

    return () => clearInterval(interval)
  }, [shouldPoll, currentSessionId, fetchTodoList])

  // A todoUpdate in the final chunk lands after the poll stops, so pick it up
  // once when streaming finishes.
  const wasStreamingRef = useRef(false)
  useEffect(() => {
    if (wasStreamingRef.current && !isStreaming && hasMessages) {
      fetchTodoList({ silent: true })
    }
    wasStreamingRef.current = !!isStreaming
  }, [isStreaming, hasMessages, fetchTodoList])

  const handleOpen = useCallback(
    async (list?: TodoList | null) => {
      // Don't open if there are no messages
      if (!messages || messages.length === 0) {
        return
      }

      if (list) {
        setTodoList(list)
      } else {
        // Fetch from API if no data provided
        await fetchTodoList()
      }
      setShow(true)
    },
    [fetchTodoList, messages]
  )

  const handleClose = useCallback(() => {
    setShow(false)
    setError(null)
  }, [])

  const handleRefresh = useCallback(() => {
    fetchTodoList()
  }, [fetchTodoList])

  return {
    show,
    todoList,
    loading,
    error,
    flash,
    handleOpen,
    handleClose,
    fetchTodoList,
    handleRefresh,
    TodoModal: (props: Omit<TodoModalProps, 'todoList' | 'onRefresh' | 'loading'>) => (
      <TodoModal {...props} todoList={todoList} onRefresh={handleRefresh} loading={loading} />
    )
  }
}

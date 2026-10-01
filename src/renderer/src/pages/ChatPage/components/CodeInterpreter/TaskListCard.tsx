import React, { useState } from 'react'
import { FaList, FaClock, FaDocker, FaCheck, FaTimes } from 'react-icons/fa'
import { MdCancel } from 'react-icons/md'
import { AsyncTaskCard, TaskStatus } from './AsyncTaskCard'

export interface TaskListResult {
  success: boolean
  name: 'codeInterpreter'
  operation: 'list'
  tasks: TaskInfo[]
  summary: {
    total: number
    pending: number
    running: number
    completed: number
    failed: number
    cancelled: number
  }
  message: string
}

export interface TaskInfo {
  taskId: string
  status: TaskStatus
  createdAt: Date
  startedAt?: Date
  completedAt?: Date
  code: string
  environment: 'basic' | 'datascience'
  inputFiles?: Array<{ path: string }>
  result?: any
  error?: string
  progress?: number
}

interface TaskListCardProps {
  result: TaskListResult
}

const statusIcons = {
  pending: { icon: FaClock, color: 'text-warning' },
  running: { icon: FaDocker, color: 'text-accent' },
  completed: { icon: FaCheck, color: 'text-success' },
  failed: { icon: FaTimes, color: 'text-danger' },
  cancelled: { icon: MdCancel, color: 'text-ink-muted' }
}

export const TaskListCard: React.FC<TaskListCardProps> = ({ result }) => {
  const [filterStatus, setFilterStatus] = useState<TaskStatus | 'all'>('all')
  const [isExpanded, setIsExpanded] = useState(true)

  // Filter tasks based on selected status
  const filteredTasks =
    filterStatus === 'all'
      ? result.tasks
      : result.tasks.filter((task) => task.status === filterStatus)

  // Convert TaskInfo to AsyncTaskInfo format for AsyncTaskCard
  const convertToAsyncTaskInfo = (task: TaskInfo) => ({
    taskId: task.taskId,
    status: task.status,
    message: `Task ${task.status}`,
    progress: task.progress,
    createdAt: task.createdAt.toISOString(),
    startedAt: task.startedAt?.toISOString(),
    completedAt: task.completedAt?.toISOString(),
    executionResult: task.result
  })

  if (!result.success) {
    return (
      <div className="rounded-container border border-danger bg-danger-soft p-2.5 mb-2">
        <div className="flex items-center gap-3 mb-2">
          <FaTimes className="w-4 h-4 text-danger" />
          <span className="font-medium text-sm text-ink">Code Interpreter Task List - Error</span>
        </div>
        <div className="text-sm text-danger">{result.message}</div>
      </div>
    )
  }

  return (
    <div className="rounded-container border border-subtle bg-surface p-2.5 mb-2">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <FaList className="w-4 h-4 text-accent" />
          <div>
            <span className="font-medium text-sm text-ink">Code Interpreter Task List</span>
            <div className="text-xs text-ink-muted">{result.message}</div>
          </div>
        </div>

        <button
          onClick={() => setIsExpanded(!isExpanded)}
          className="text-xs text-ink-muted hover:text-ink"
        >
          {isExpanded ? '▼' : '▶'} {isExpanded ? 'Collapse' : 'Expand'}
        </button>
      </div>

      {/* Summary Stats - Clickable Filters */}
      <div className="grid grid-cols-3 md:grid-cols-6 gap-2 mb-4 text-xs">
        {Object.entries(result.summary).map(([status, count]) => {
          const statusKey = status as keyof typeof statusIcons
          const statusConfig = statusIcons[statusKey]
          if (!statusConfig || status === 'total') return null

          const StatusIcon = statusConfig.icon
          const isActive = filterStatus === status
          return (
            <button
              key={status}
              onClick={() => setFilterStatus(status as TaskStatus)}
              className={`flex items-center gap-1 px-2 py-1 rounded-control transition-colors cursor-pointer hover:shadow-sm ${
                isActive ? 'bg-accent-tint ring-1 ring-accent' : 'bg-surface-2 hover:bg-raised'
              }`}
            >
              <StatusIcon className={`w-3 h-3 ${statusConfig.color}`} />
              <span className="capitalize text-ink">{status}</span>
              <span className="font-medium text-ink">{count}</span>
            </button>
          )
        })}
        <button
          onClick={() => setFilterStatus('all')}
          className={`flex items-center gap-1 px-2 py-1 rounded-control transition-colors cursor-pointer hover:shadow-sm ${
            filterStatus === 'all'
              ? 'bg-accent-tint ring-1 ring-accent'
              : 'bg-accent-tint hover:bg-accent-tint-strong'
          }`}
        >
          <span className="text-accent">Total</span>
          <span className="font-medium text-accent">{result.summary.total}</span>
        </button>
      </div>

      {isExpanded && (
        <>
          {/* Task List */}
          {filteredTasks.length === 0 ? (
            <div className="text-center py-6 text-ink-muted">
              <FaList className="w-8 h-8 mx-auto mb-2 opacity-50" />
              <div className="text-sm">No tasks found</div>
              {filterStatus !== 'all' && (
                <button
                  onClick={() => setFilterStatus('all')}
                  className="text-xs text-accent hover:text-accent mt-1"
                >
                  Show all tasks
                </button>
              )}
            </div>
          ) : (
            <div className="space-y-2 max-h-96 overflow-y-auto">
              {filteredTasks.map((task) => (
                <AsyncTaskCard key={task.taskId} taskInfo={convertToAsyncTaskInfo(task)} />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  )
}

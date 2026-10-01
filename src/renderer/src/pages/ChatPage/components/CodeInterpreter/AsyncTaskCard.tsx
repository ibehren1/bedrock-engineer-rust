import React, { useState } from 'react'
import { FaClock, FaCheck, FaTimes, FaDocker } from 'react-icons/fa'
import { MdCancel } from 'react-icons/md'

import { CodeInterpreterResult } from '../CodeBlocks/CodeInterpreter/CodeInterpreterResult'

export type TaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled'

export interface AsyncTaskInfo {
  taskId: string
  status: TaskStatus
  message: string
  progress?: number
  createdAt: string
  startedAt?: string
  completedAt?: string
  executionResult?: any
}

interface AsyncTaskCardProps {
  taskInfo: AsyncTaskInfo
}

const statusConfig = {
  pending: {
    icon: FaClock,
    color: 'text-warning',
    bgColor: 'bg-warning-soft',
    borderColor: 'border-warning',
    label: 'Pending'
  },
  running: {
    icon: FaDocker,
    color: 'text-accent',
    bgColor: 'bg-accent-tint',
    borderColor: 'border-accent',
    label: 'Running'
  },
  completed: {
    icon: FaCheck,
    color: 'text-success',
    bgColor: 'bg-success-soft',
    borderColor: 'border-success',
    label: 'Completed'
  },
  failed: {
    icon: FaTimes,
    color: 'text-danger',
    bgColor: 'bg-danger-soft',
    borderColor: 'border-danger',
    label: 'Failed'
  },
  cancelled: {
    icon: MdCancel,
    color: 'text-ink-muted',
    bgColor: 'bg-surface-2',
    borderColor: 'border-subtle',
    label: 'Cancelled'
  }
}

// Convert executionResult to CodeInterpreterResult format
const convertToCodeInterpreterResult = (executionResult: any) => {
  if (!executionResult) return null

  return {
    success: executionResult.success !== false,
    name: 'codeInterpreter',
    code: executionResult.code || '',
    message: executionResult.message || '',
    output: executionResult.output || '',
    error: executionResult.error,
    executionTime: executionResult.executionTime || 0,
    result: {
      code: executionResult.code || '',
      stdout: executionResult.stdout || executionResult.output || '',
      stderr: executionResult.stderr || executionResult.error || '',
      exitCode: executionResult.exitCode || (executionResult.success !== false ? 0 : 1),
      files: executionResult.files || []
    }
  }
}

export const AsyncTaskCard: React.FC<AsyncTaskCardProps> = ({ taskInfo }) => {
  const [isExpanded, setIsExpanded] = useState(false)

  const config = statusConfig[taskInfo.status]
  const displayConfig = statusConfig[taskInfo.status] || config
  const StatusIcon = displayConfig.icon

  const formatDuration = (start?: string, end?: string) => {
    if (!start) return null
    const startTime = new Date(start)
    const endTime = end ? new Date(end) : new Date()
    const duration = Math.floor((endTime.getTime() - startTime.getTime()) / 1000)

    if (duration < 60) return `${duration}s`
    if (duration < 3600) return `${Math.floor(duration / 60)}m ${duration % 60}s`
    return `${Math.floor(duration / 3600)}h ${Math.floor((duration % 3600) / 60)}m`
  }

  const codeInterpreterResult = convertToCodeInterpreterResult(taskInfo.executionResult)

  // If completed and has execution result, show detailed result
  if (taskInfo.status === 'completed' && codeInterpreterResult) {
    return <CodeInterpreterResult response={codeInterpreterResult} />
  }

  return (
    <div
      className={`rounded-container border ${displayConfig.borderColor} ${displayConfig.bgColor} p-2.5 mb-2`}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-3">
          <StatusIcon className={`w-4 h-4 ${displayConfig.color}`} />
          <div>
            <span className="font-medium text-sm text-ink">Code Interpreter Execution</span>
            <div className="text-xs text-ink-muted">
              {displayConfig.label} • {taskInfo.taskId.substring(0, 12)}...
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* Progress bar for running tasks */}
          {taskInfo.status === 'running' && taskInfo.progress !== undefined && (
            <div className="w-20 bg-raised rounded-full h-2">
              <div
                className="bg-accent h-2 rounded-full transition-all duration-300"
                style={{ width: `${taskInfo.progress}%` }}
              />
            </div>
          )}
        </div>
      </div>

      {/* Status message */}
      <div className="text-sm text-ink mb-2">{taskInfo.message}</div>

      {/* Timing information */}
      <div className="flex items-center gap-4 text-xs text-ink-muted">
        <span>Created: {new Date(taskInfo.createdAt).toLocaleTimeString()}</span>
        {taskInfo.startedAt && (
          <span>Started: {new Date(taskInfo.startedAt).toLocaleTimeString()}</span>
        )}
        {taskInfo.completedAt && (
          <span>Completed: {new Date(taskInfo.completedAt).toLocaleTimeString()}</span>
        )}
        {taskInfo.startedAt && (
          <span>Duration: {formatDuration(taskInfo.startedAt, taskInfo.completedAt)}</span>
        )}
      </div>

      {/* Expandable details for non-completed tasks */}
      {!codeInterpreterResult && (taskInfo.executionResult || isExpanded) && (
        <div className="mt-3 pt-3 border-t border-subtle">
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="text-xs text-ink-muted hover:text-ink mb-2"
          >
            {isExpanded ? '▼' : '▶'} Details
          </button>

          {isExpanded && (
            <div className="text-xs">
              <div className="bg-raised rounded-control p-2 font-mono">
                Task ID: {taskInfo.taskId}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

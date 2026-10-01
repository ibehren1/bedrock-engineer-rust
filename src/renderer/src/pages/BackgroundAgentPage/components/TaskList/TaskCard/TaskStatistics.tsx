import React from 'react'

interface TaskStatisticsProps {
  runCount: number
  lastRun?: number
  onViewHistory: () => void
  formatDate: (timestamp?: number) => string
  // i18n labels
  executionCountLabel: string
  lastRunLabel: string
  viewHistoryLabel: string
  historyButtonTitle: string
}

export const TaskStatistics: React.FC<TaskStatisticsProps> = ({
  runCount,
  lastRun,
  onViewHistory,
  formatDate,
  executionCountLabel,
  lastRunLabel,
  viewHistoryLabel,
  historyButtonTitle
}) => {
  return (
    <div className="mb-3">
      <div className="flex items-center justify-between text-sm">
        <div className="flex items-center space-x-4 text-ink-muted">
          <span className="flex items-center space-x-1">
            <span className="font-medium text-ink">{runCount}</span>
            <span>{executionCountLabel}</span>
          </span>
          <span className="flex items-center space-x-1">
            <span>{lastRunLabel}:</span>
            <span className="font-medium text-ink">{formatDate(lastRun)}</span>
          </span>
        </div>
        <button
          onClick={onViewHistory}
          className="text-xs text-accent hover:text-accent transition-colors"
          title={historyButtonTitle}
        >
          {viewHistoryLabel}
        </button>
      </div>
    </div>
  )
}

import React from 'react'
import {
  ChevronDownIcon,
  ChevronUpIcon,
  CalendarIcon,
  ClockIcon
} from '@heroicons/react/24/outline'
import { StatusBadge } from '../atoms/StatusBadge'

interface TaskExpandedDetailsProps {
  isExpanded: boolean
  onToggleExpanded: () => void
  wakeWord: string
  continueSession?: boolean
  continueSessionPrompt?: string
  createdAt?: number
  nextRun?: number
  enabled: boolean
  formatDate: (timestamp?: number) => string
  // i18n labels
  detailsLabel: string
  continuationLabel: string
  wakeWordLabel: string
  continuationPromptLabel: string
  createdLabel: string
  nextRunLabel: string
}

export const TaskExpandedDetails: React.FC<TaskExpandedDetailsProps> = ({
  isExpanded,
  onToggleExpanded,
  wakeWord,
  continueSession,
  continueSessionPrompt,
  createdAt,
  nextRun,
  enabled,
  formatDate,
  detailsLabel,
  continuationLabel,
  wakeWordLabel,
  continuationPromptLabel,
  createdLabel,
  nextRunLabel
}) => {
  return (
    <div className="mb-3">
      <div className="flex items-center space-x-3">
        <button
          onClick={onToggleExpanded}
          className="text-xs text-ink-muted hover:text-accent transition-colors flex items-center space-x-1"
        >
          <span>{detailsLabel}</span>
          {isExpanded ? (
            <ChevronUpIcon className="h-3 w-3" />
          ) : (
            <ChevronDownIcon className="h-3 w-3" />
          )}
        </button>
        {continueSession && <StatusBadge type="info">{continuationLabel}</StatusBadge>}
      </div>

      {/* Expandable Details */}
      {isExpanded && (
        <div className="mt-3 space-y-3">
          {/* Wake Word */}
          <div>
            <div className="text-xs font-medium text-ink mb-1">{wakeWordLabel}:</div>
            <div className="bg-surface-2 rounded-control p-2 text-xs text-ink-muted max-h-24 overflow-y-auto">
              {wakeWord}
            </div>
          </div>

          {/* Session Continuation */}
          {continueSession && continueSessionPrompt && (
            <div>
              <div className="text-xs font-medium text-accent mb-1">{continuationPromptLabel}:</div>
              <div className="bg-accent-tint rounded-control p-2 text-xs text-ink-muted max-h-24 overflow-y-auto">
                {continueSessionPrompt}
              </div>
            </div>
          )}

          {/* Created Date */}
          <div className="flex items-center space-x-2 text-xs text-ink-muted">
            <CalendarIcon className="h-3 w-3" />
            <span>
              {createdLabel}: {formatDate(createdAt)}
            </span>
          </div>

          {/* Next Run (only meaningful for enabled tasks) */}
          {enabled && nextRun && (
            <div className="flex items-center space-x-2 text-xs text-ink-muted">
              <ClockIcon className="h-3 w-3" />
              <span>
                {nextRunLabel}: {formatDate(nextRun)}
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

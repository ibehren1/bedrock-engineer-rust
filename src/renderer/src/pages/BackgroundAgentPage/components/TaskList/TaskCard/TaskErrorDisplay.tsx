import React from 'react'
import { XCircleIcon } from '@heroicons/react/24/outline'

interface TaskErrorDisplayProps {
  error: string
  errorTitle: string
}

export const TaskErrorDisplay: React.FC<TaskErrorDisplayProps> = ({ error, errorTitle }) => {
  return (
    <div className="mt-4 p-3 bg-danger-soft border border-danger rounded-control">
      <div className="flex items-start space-x-2">
        <XCircleIcon className="h-4 w-4 text-danger mt-0.5 flex-shrink-0" />
        <div>
          <div className="text-sm font-medium text-danger">{errorTitle}:</div>
          <div className="text-sm text-danger mt-1">{error}</div>
        </div>
      </div>
    </div>
  )
}

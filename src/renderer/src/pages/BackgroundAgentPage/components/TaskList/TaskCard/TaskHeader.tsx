import React from 'react'
import { StatusBadge } from '../atoms/StatusBadge'

interface TaskHeaderProps {
  name: string
  hasError?: boolean
}

export const TaskHeader: React.FC<TaskHeaderProps> = ({ name, hasError }) => {
  return (
    <div className="flex items-center space-x-2 mb-2">
      <h3 className="text-heading font-medium text-ink">{name}</h3>
      {/* Show status badge only for error state */}
      {hasError && <StatusBadge type="error">エラー</StatusBadge>}
    </div>
  )
}

import React from 'react'
import { XCircleIcon } from '@heroicons/react/24/outline'

interface StatusBadgeProps {
  type: 'error' | 'success' | 'warning' | 'info'
  children: React.ReactNode
  icon?: React.ReactNode
}

export const StatusBadge: React.FC<StatusBadgeProps> = ({ type, children, icon }) => {
  const getTypeClasses = () => {
    switch (type) {
      case 'error':
        return 'bg-danger-soft text-danger'
      case 'success':
        return 'bg-success-soft text-success'
      case 'warning':
        return 'bg-warning-soft text-warning'
      case 'info':
        return 'bg-accent-tint text-accent'
      default:
        return 'bg-raised text-ink'
    }
  }

  const defaultIcon = type === 'error' ? <XCircleIcon className="h-3 w-3 mr-1" /> : null

  return (
    <span
      className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${getTypeClasses()}`}
    >
      {icon || defaultIcon}
      {children}
    </span>
  )
}

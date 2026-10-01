import React from 'react'
import { motion } from 'framer-motion'
import { Squares2X2Icon, TableCellsIcon } from '@heroicons/react/24/outline'

interface TaskViewToggleProps {
  isTableView: boolean
  onToggle: (isTableView: boolean) => void
  className?: string
}

export const TaskViewToggle: React.FC<TaskViewToggleProps> = ({
  isTableView,
  onToggle,
  className = ''
}) => {
  return (
    <div
      className={`
        relative inline-flex items-center
        bg-raised
        rounded-control p-1
        border border-subtle
        cursor-pointer
        ${className}
      `}
    >
      {/* アニメーション背景 */}
      <motion.div
        className="absolute top-0.5 h-6 w-8 bg-surface/60 rounded-control shadow-sm"
        animate={{
          x: isTableView ? 32 : 0
        }}
        transition={{
          type: 'spring',
          stiffness: 300,
          damping: 30,
          duration: 0.3
        }}
      />

      {/* LIST ボタン (カードアイコン) */}
      <button
        onClick={() => onToggle(false)}
        className={`
          relative z-10 flex items-center justify-center
          w-8 h-5 rounded-control
          transition-colors duration-200
          ${!isTableView ? 'text-ink' : 'text-ink-faint hover:text-ink-muted'}
        `}
        title="Switch to List View"
      >
        <Squares2X2Icon className="w-4 h-4" />
      </button>

      {/* TABLE ボタン (テーブルアイコン) */}
      <button
        onClick={() => onToggle(true)}
        className={`
          relative z-10 flex items-center justify-center
          w-8 h-5 rounded-control
          transition-colors duration-200
          ${isTableView ? 'text-ink' : 'text-ink-faint hover:text-ink-muted'}
        `}
        title="Switch to Table View"
      >
        <TableCellsIcon className="w-4 h-4" />
      </button>
    </div>
  )
}

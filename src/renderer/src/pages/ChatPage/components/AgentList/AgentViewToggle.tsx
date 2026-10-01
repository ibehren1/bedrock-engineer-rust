import React from 'react'
import { motion } from 'framer-motion'
import { Tooltip } from 'flowbite-react'
import { HiViewGrid, HiViewList } from 'react-icons/hi'

interface AgentViewToggleProps {
  viewMode: 'card' | 'table'
  onToggle: (viewMode: 'card' | 'table') => void
  className?: string
}

export const AgentViewToggle: React.FC<AgentViewToggleProps> = ({
  viewMode,
  onToggle,
  className = ''
}) => {
  return (
    <div
      className={`
        relative inline-flex items-center
        bg-raised
        rounded-control p-0.5
        border border-subtle
        cursor-pointer
        ${className}
      `}
    >
      {/* Animation background */}
      <motion.div
        className="absolute top-0.5 h-8 w-10 bg-surface/60 rounded-control shadow-sm"
        animate={{
          x: viewMode === 'table' ? 40 : 0
        }}
        transition={{
          type: 'spring',
          stiffness: 300,
          damping: 30,
          duration: 0.3
        }}
      />

      {/* Card View button (Grid icon) */}
      <Tooltip content="Card" placement="bottom" animation="duration-500">
        <button
          onClick={() => onToggle('card')}
          className={`
            relative z-10 flex items-center justify-center
            w-10 h-8 rounded-control
            transition-colors duration-200
            ${viewMode === 'card' ? 'text-ink' : 'text-ink-faint hover:text-ink-muted'}
          `}
        >
          <HiViewGrid size={18} />
        </button>
      </Tooltip>

      {/* Table View button (List icon) */}
      <Tooltip content="Table" placement="bottom" animation="duration-500">
        <button
          onClick={() => onToggle('table')}
          className={`
            relative z-10 flex items-center justify-center
            w-10 h-5 rounded-control
            transition-colors duration-200
            ${viewMode === 'table' ? 'text-ink' : 'text-ink-faint hover:text-ink-muted'}
          `}
        >
          <HiViewList size={18} />
        </button>
      </Tooltip>
    </div>
  )
}

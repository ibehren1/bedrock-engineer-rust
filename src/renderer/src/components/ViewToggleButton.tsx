import React from 'react'
import { motion } from 'framer-motion'
import { HiMicrophone, HiChatBubbleLeftRight } from 'react-icons/hi2'

interface ViewToggleButtonProps {
  isDetailView: boolean
  onToggle: (isDetailView: boolean) => void
  className?: string
}

export const ViewToggleButton: React.FC<ViewToggleButtonProps> = ({
  isDetailView,
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
      {/* アニメーション背景 */}
      <motion.div
        className="absolute top-0.5 h-5 w-8 bg-surface/60 rounded-control shadow-sm"
        animate={{
          x: isDetailView ? 32 : 0
        }}
        transition={{
          type: 'spring',
          stiffness: 300,
          damping: 30,
          duration: 0.3
        }}
      />

      {/* SIMPLE ボタン (マイクアイコン) */}
      <button
        onClick={() => onToggle(false)}
        className={`
          relative z-10 flex items-center justify-center
          w-8 h-5 rounded-control
          transition-colors duration-200
          ${!isDetailView ? 'text-ink' : 'text-ink-faint hover:text-ink-muted'}
        `}
        title="Switch to Simple View"
      >
        <HiMicrophone size={12} />
      </button>

      {/* DETAIL ボタン (チャットアイコン) */}
      <button
        onClick={() => onToggle(true)}
        className={`
          relative z-10 flex items-center justify-center
          w-8 h-5 rounded-control
          transition-colors duration-200
          ${isDetailView ? 'text-ink' : 'text-ink-faint hover:text-ink-muted'}
        `}
        title="Switch to Detail View"
      >
        <HiChatBubbleLeftRight size={12} />
      </button>
    </div>
  )
}

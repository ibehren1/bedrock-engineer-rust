import { motion } from 'framer-motion'
import { AiOutlineCloud, AiOutlineCode, AiOutlineProject } from 'react-icons/ai'

export type DiagramMode = 'aws' | 'software-architecture' | 'business-process'

interface DiagramModeOption {
  id: DiagramMode
  label: string
  description: string
  icon: JSX.Element
}

const modeOptions: DiagramModeOption[] = [
  {
    id: 'aws',
    label: 'Cloud',
    description: 'AWS architecture diagrams',
    icon: <AiOutlineCloud className="w-4 h-4" />
  },
  {
    id: 'software-architecture',
    label: 'Software',
    description: 'Software architecture & database design diagrams',
    icon: <AiOutlineCode className="w-4 h-4" />
  },
  {
    id: 'business-process',
    label: 'Business',
    description: 'Business process & workflow diagrams',
    icon: <AiOutlineProject className="w-4 h-4" />
  }
]

interface DiagramModeSelectorProps {
  selectedMode: DiagramMode
  onModeChange: (mode: DiagramMode) => void
  onRefresh?: () => void
}

export function DiagramModeSelector({
  selectedMode,
  onModeChange,
  onRefresh
}: DiagramModeSelectorProps) {
  const handleModeChange = (mode: DiagramMode) => {
    if (mode !== selectedMode) {
      onModeChange(mode)
      // モード変更時にページをリフレッシュ
      if (onRefresh) {
        onRefresh()
      }
    }
  }

  return (
    <div className="flex gap-2">
      {modeOptions.map((mode) => (
        <motion.button
          key={mode.id}
          initial={{ opacity: 0, scale: 0.8 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.3 }}
          className={`
            text-ink
            ${selectedMode === mode.id ? 'bg-success-soft' : 'bg-surface'}
            hover:bg-success-soft-strong
            border
            ${selectedMode === mode.id ? 'border-success' : 'border-subtle'}
            focus:ring-4
            focus:outline-none
            focus:ring-accent
            font-medium
            rounded-full
            text-xs
            px-3
            py-1.5
            inline-flex
            items-center
            flex
            gap-2
            bg-surface
            text-ink
            border-subtle
            hover:bg-raised
          `}
          onClick={() => handleModeChange(mode.id)}
        >
          <div className="w-[18px]">{mode.icon}</div>
          <span>{mode.label}</span>
        </motion.button>
      ))}
    </div>
  )
}

import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiPlus, FiX } from 'react-icons/fi'
import type { LocalTerminalInfo } from '@/types/bridge/api'
import { LocalTerminalView } from './LocalTerminalView'

type LocalTerminalPanelProps = {
  terminals: LocalTerminalInfo[]
  selectedId?: string
  onSelect: (terminalId: string) => void
  onNew: () => void
  onClose: (terminalId: string) => void
  onExit: (terminalId: string, exitCode: number | null) => void
  /** The Terminal tab is showing. */
  active: boolean
}

/** The chat page's Terminal tab: the chat's local shells, one sub-tab each. */
export const LocalTerminalPanel: React.FC<LocalTerminalPanelProps> = ({
  terminals,
  selectedId,
  onSelect,
  onNew,
  onClose,
  onExit,
  active
}) => {
  const { t } = useTranslation()
  const selected =
    terminals.find((item) => item.terminalId === selectedId) ?? terminals[terminals.length - 1]

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center gap-0.5 px-2 border-b border-subtle min-w-0">
        {terminals.map((item, index) => {
          const isSelected = item.terminalId === selected?.terminalId
          return (
            <div
              key={item.terminalId}
              className={`group inline-flex items-center -mb-px border-b-2 ${
                isSelected ? 'border-accent' : 'border-transparent'
              }`}
            >
              <button
                onClick={() => onSelect(item.terminalId)}
                title={item.cwd}
                className={`pl-2.5 pr-1 py-1 text-xs whitespace-nowrap ${
                  isSelected ? 'text-ink' : 'text-ink-faint hover:text-ink'
                } ${item.exited ? 'line-through' : ''}`}
              >
                {item.shell} {index + 1}
              </button>
              <button
                onClick={() => onClose(item.terminalId)}
                title={t('filesPanel.terminalClose')}
                aria-label={t('filesPanel.terminalClose')}
                className="mr-1 p-0.5 rounded-control text-ink-faint hover:text-ink hover:bg-raised"
              >
                <FiX className="size-3" />
              </button>
            </div>
          )
        })}
        <button
          onClick={onNew}
          title={t('filesPanel.terminalNew')}
          aria-label={t('filesPanel.terminalNew')}
          className="ml-1 p-1 rounded-control text-ink-muted hover:text-ink hover:bg-raised"
        >
          <FiPlus className="size-3.5" />
        </button>
        {selected && (
          <span
            className="ml-auto pl-3 text-micro font-mono text-ink-faint truncate min-w-0"
            title={selected.cwd}
          >
            {selected.cwd}
          </span>
        )}
      </div>

      {/* Every shell keeps its view (and scrollback); only the selected one is visible. */}
      <div className="relative flex-1 min-h-0">
        {terminals.map((item) => {
          const isSelected = item.terminalId === selected?.terminalId
          return (
            <div
              key={item.terminalId}
              className={`absolute inset-0 ${isSelected ? '' : 'invisible pointer-events-none'}`}
            >
              <LocalTerminalView
                info={item}
                active={active && isSelected}
                onExit={(code) => onExit(item.terminalId, code)}
                onClose={() => onClose(item.terminalId)}
              />
            </div>
          )
        })}
      </div>
    </div>
  )
}

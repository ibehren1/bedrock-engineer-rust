import React, { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { FiChevronDown, FiEdit3 } from 'react-icons/fi'
import { TbRobot } from 'react-icons/tb'
import { CustomAgent } from '@/types/agent-chat'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { EXCLUDED_CHAT_AGENT_IDS } from '../AgentList'

type AgentSelectorProps = {
  agents: readonly CustomAgent[]
  /** Selected agent id. Omit to follow the globally selected agent. */
  value?: string
  /** Called instead of updating the global selection when provided. */
  onChange?: (agentId: string) => void
  alignment?: 'left' | 'center'
  /** Show the "Edit agents" entry that navigates to the My Agents page */
  showEditAgentsLink?: boolean
  /** Open upward (default, for the message entry bar) or downward (headers, forms) */
  openDirection?: 'up' | 'down'
  className?: string
}

const agentIcon = (agent?: CustomAgent, className = 'w-4 h-4') => {
  if (!agent?.icon) {
    return <TbRobot className={className} />
  }
  return (
    <AgentIconView
      icon={agent.icon}
      iconColor={agent.iconColor}
      className={className}
      fallback={<TbRobot className={className} />}
    />
  )
}

export const AgentSelector: React.FC<AgentSelectorProps> = ({
  agents,
  value,
  onChange,
  alignment = 'center',
  showEditAgentsLink = true,
  openDirection = 'up',
  className
}) => {
  const [isOpen, setIsOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const navigate = useNavigate()
  const { t } = useTranslation()
  const { selectedAgentId, setSelectedAgentId } = useSettings()

  const currentAgentId = value ?? selectedAgentId
  const selectedAgent = agents.find((agent) => agent.id === currentAgentId)
  const selectableAgents = agents.filter(
    (agent) => !!agent.id && !EXCLUDED_CHAT_AGENT_IDS.includes(agent.id)
  )

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const handleSelect = (agentId: string) => {
    if (onChange) {
      onChange(agentId)
    } else {
      setSelectedAgentId(agentId)
    }
    setIsOpen(false)
  }

  const containerClass = alignment === 'left' ? 'justify-start' : 'justify-center'
  const popoverPosition = openDirection === 'up' ? 'bottom-full mb-1' : 'top-full mt-1'

  return (
    <div
      className={`${containerClass} flex items-center relative ${className || ''}`}
      ref={dropdownRef}
    >
      <div className="relative">
        {isOpen && (
          <div
            className={`absolute z-20 w-[25rem] ${popoverPosition} bg-surface rounded-container shadow-lg
            border border-subtle py-2 px-2 max-h-[40vh] overflow-y-auto`}
          >
            {selectableAgents.map((agent) => (
              <div
                key={agent.id}
                onClick={() => handleSelect(agent.id!)}
                className={`
                  flex items-center gap-4 px-3 py-2.5 cursor-pointer rounded-control transition-colors
                  ${agent.id === currentAgentId ? 'bg-surface-2' : 'bg-surface'}
                  hover:bg-raised
                `}
              >
                <span className="text-ink-muted">{agentIcon(agent)}</span>
                <div className="flex flex-col min-w-0">
                  <span className="font-medium text-ink flex items-center gap-2">
                    <span className="truncate">{agent.name}</span>
                    {agent.isShared && (
                      <span className="px-2 py-0.5 text-xs font-medium text-success bg-success-soft rounded-control">
                        {t('shared')}
                      </span>
                    )}
                  </span>
                  <span className="text-xs text-ink-muted mt-0.5 line-clamp-2">
                    {t(agent.description) || t('noDescription')}
                  </span>
                </div>
              </div>
            ))}

            {showEditAgentsLink && (
              <div
                onClick={() => {
                  setIsOpen(false)
                  navigate('/my-agents')
                }}
                className="flex items-center gap-2 mt-1 px-3 py-2.5 cursor-pointer rounded-control
                  border-t border-subtle text-sm text-accent
                  hover:bg-surface-2 transition-colors"
              >
                <FiEdit3 className="w-4 h-4" />
                <span>{t('myAgents.editAgents')}</span>
              </div>
            )}
          </div>
        )}

        <button
          type="button"
          onClick={() => setIsOpen(!isOpen)}
          className="flex items-center gap-1 text-sm text-ink-muted rounded-control transition-colors"
        >
          <span className="flex items-center gap-1.5">
            <span className="text-ink-muted">{agentIcon(selectedAgent, 'w-4 h-4')}</span>
            <span className="text-left whitespace-nowrap">{selectedAgent?.name}</span>
            <FiChevronDown className="text-ink-faint" size={16} />
          </span>
        </button>
      </div>
    </div>
  )
}

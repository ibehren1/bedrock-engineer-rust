import React from 'react'
import { Dropdown } from 'flowbite-react'
import { useTranslation } from 'react-i18next'
import { FiChevronDown } from 'react-icons/fi'
import { TbRobot } from 'react-icons/tb'
import { CustomAgent } from '@/types/agent-chat'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'

interface UnhideAgentsDropdownProps {
  /** Built-in agents the user hid; the dropdown is not rendered when empty */
  hiddenAgents: CustomAgent[]
  onUnhideAgent: (agentId: string) => void
  onUnhideAll: () => void
}

/**
 * Lists the hidden built-in agents so they can be unhidden one at a time, with
 * an "unhide all" entry for bringing the whole set back.
 */
export const UnhideAgentsDropdown: React.FC<UnhideAgentsDropdownProps> = ({
  hiddenAgents,
  onUnhideAgent,
  onUnhideAll
}) => {
  const { t } = useTranslation()

  if (hiddenAgents.length === 0) {
    return null
  }

  return (
    <Dropdown
      label=""
      dismissOnClick={true}
      renderTrigger={() => (
        <button
          className="px-2 py-1 text-sm font-medium text-ink bg-surface
            border border-strong rounded-container shadow-raised
            hover:bg-raised focus:outline-none focus:ring-2
            focus:ring-offset-2 focus:ring-accent
            whitespace-nowrap flex gap-1.5 items-center"
        >
          {t('myAgents.unhide', { count: hiddenAgents.length })}
          <FiChevronDown className="w-4 h-4" />
        </button>
      )}
    >
      <Dropdown.Header>
        <span className="text-xs font-medium text-ink-muted">{t('myAgents.unhideHint')}</span>
      </Dropdown.Header>
      {hiddenAgents.map((agent) => (
        <Dropdown.Item
          key={agent.id}
          onClick={() => onUnhideAgent(agent.id!)}
          className="w-64 flex gap-2 items-center"
        >
          <AgentIconView
            icon={agent.icon}
            iconColor={agent.iconColor}
            className="w-4 h-4 shrink-0"
            fallback={<TbRobot className="w-4 h-4 shrink-0" />}
          />
          <span className="truncate">{agent.name}</span>
        </Dropdown.Item>
      ))}
      <Dropdown.Divider />
      <Dropdown.Item onClick={onUnhideAll} className="w-64">
        {t('myAgents.unhideAll')}
      </Dropdown.Item>
    </Dropdown>
  )
}

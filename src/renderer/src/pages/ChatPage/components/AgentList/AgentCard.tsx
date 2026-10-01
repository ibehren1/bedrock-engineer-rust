import React from 'react'
import { CustomAgent } from '@/types/agent-chat'
import { TbRobot } from 'react-icons/tb'
import { MdDragIndicator } from 'react-icons/md'
import { useTranslation } from 'react-i18next'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import { AgentActionsDropdown } from './AgentActionsDropdown'

interface AgentCardProps {
  agent: CustomAgent
  isCustomAgent: boolean
  isSelected: boolean
  onSelect: (agentId: string) => void
  onEdit?: (agent: CustomAgent) => void
  onDuplicate?: (agent: CustomAgent) => void
  onDelete?: (agentId: string) => void
  onSaveAsShared?: (agent: CustomAgent) => void
  onDeleteSharedFile?: (agent: CustomAgent) => void
  onDownloadYaml?: (agent: CustomAgent) => void
  onShareToOrganization?: (agent: CustomAgent) => void
  onConvertToStrands?: (agentId: string) => void
  /** HTML5 drag handlers from useAgentDragOrder; empty when reordering is off */
  dragProps?: React.HTMLAttributes<HTMLDivElement> & { draggable?: boolean }
  dragClassName?: string
}

export const AgentCard: React.FC<AgentCardProps> = ({
  agent,
  isCustomAgent,
  isSelected,
  onSelect,
  onEdit,
  onDuplicate,
  onDelete,
  onSaveAsShared,
  onDeleteSharedFile,
  onDownloadYaml,
  onShareToOrganization,
  onConvertToStrands,
  dragProps,
  dragClassName
}) => {
  const { t } = useTranslation()

  return (
    <div
      className={`group relative flex items-start p-2.5 border
        border-subtle
        rounded-container bg-surface hover:border-accent
        hover:border-accent transition-all duration-200 cursor-pointer
        ${dragClassName || ''}`}
      onClick={() => onSelect(agent.id!)}
      {...dragProps}
    >
      {dragProps?.draggable && (
        <MdDragIndicator
          className="absolute left-1 top-1/2 -translate-y-1/2 w-4 h-4 text-ink-faint
            opacity-0 group-hover:opacity-100 transition-opacity"
          title={t('myAgents.dragToReorder')}
        />
      )}
      <div className="flex-shrink-0 mr-4">
        <div
          className={`w-10 h-10 flex items-center justify-center
            ${!isCustomAgent ? 'bg-surface-2' : 'bg-raised'}
            rounded-container border border-subtle`}
        >
          {agent.icon ? (
            <AgentIconView
              icon={agent.icon}
              className="w-4 h-4 text-ink"
              style={{
                color: agent.iconColor || 'var(--tw-text-gray-700)',
                filter: 'brightness(1.2) contrast(1.2)'
              }}
            />
          ) : (
            <TbRobot className="w-4 h-4 text-accent filter brightness-110 contrast-125" />
          )}
        </div>
      </div>
      <div className="flex-1 min-w-0 relative pr-10">
        <div className="flex items-center gap-2 mb-1">
          <h3 className="text-base font-medium text-ink pr-6 truncate">{agent.name}</h3>
          <div className="flex items-center gap-1">
            {isSelected && (
              <span
                title={t('myAgents.activeInChat')}
                className="px-2 py-0.5 text-xs font-medium text-accent bg-accent-tint rounded-control"
              >
                {t('active')}
              </span>
            )}
            {agent.isShared && (
              <span className="px-2 py-0.5 text-xs font-medium text-success bg-success-soft rounded-control">
                {t('shared')}
              </span>
            )}
          </div>
        </div>
        <p className="text-sm text-ink-muted line-clamp-2 break-words">
          {t(agent.description) || t('noDescription')}
        </p>
        <div className="absolute right-0 top-0">
          <AgentActionsDropdown
            agent={agent}
            onEdit={onEdit}
            onDuplicate={onDuplicate}
            onDelete={onDelete}
            onSaveAsShared={onSaveAsShared}
            onDeleteSharedFile={onDeleteSharedFile}
            onDownloadYaml={onDownloadYaml}
            onShareToOrganization={onShareToOrganization}
            onConvertToStrands={onConvertToStrands}
          />
        </div>
      </div>
    </div>
  )
}

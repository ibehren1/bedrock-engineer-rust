import React from 'react'
import { CustomAgent } from '@/types/agent-chat'
import { FiChevronUp, FiChevronDown } from 'react-icons/fi'
import { TbRobot } from 'react-icons/tb'
import { MdDragIndicator } from 'react-icons/md'
import { useTranslation } from 'react-i18next'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'
import { SortKey, SortOrder } from './useAgentFilter'
import { AgentActionsDropdown } from './AgentActionsDropdown'

interface AgentTableViewProps {
  agents: CustomAgent[]
  selectedAgentId?: string
  onSelectAgent: (agentId: string) => void
  onEditAgent: (agent: CustomAgent) => void
  onDuplicateAgent: (agent: CustomAgent) => void
  onDeleteAgent: (agentId: string) => void
  onSaveAsShared?: (agent: CustomAgent) => void
  onDeleteSharedFile?: (agent: CustomAgent) => void
  onDownloadYaml?: (agent: CustomAgent) => void
  onShareToOrganization?: (agent: CustomAgent) => void
  onConvertToStrands?: (agentId: string) => void
  sortKey: SortKey
  sortOrder: SortOrder
  onSort: (key: SortKey) => void
  /** HTML5 drag handlers from useAgentDragOrder */
  dragProps?: (agentId?: string) => React.HTMLAttributes<HTMLTableRowElement> & {
    draggable?: boolean
  }
  dragClassName?: (agentId?: string) => string
  dragEnabled?: boolean
}

export const AgentTableView: React.FC<AgentTableViewProps> = ({
  agents,
  selectedAgentId,
  onSelectAgent,
  onEditAgent,
  onDuplicateAgent,
  onDeleteAgent,
  onSaveAsShared,
  onDeleteSharedFile,
  onDownloadYaml,
  onShareToOrganization,
  onConvertToStrands,
  sortKey,
  sortOrder,
  onSort,
  dragProps,
  dragClassName,
  dragEnabled = false
}) => {
  const { t } = useTranslation()

  const SortIcon: React.FC<{ columnKey: SortKey }> = ({ columnKey }) => {
    if (sortKey !== columnKey) {
      return (
        <div className="flex flex-col opacity-30">
          <FiChevronUp className="w-3 h-3 -mb-1" />
          <FiChevronDown className="w-3 h-3" />
        </div>
      )
    }
    return sortOrder === 'asc' ? (
      <FiChevronUp className="w-4 h-4" />
    ) : (
      <FiChevronDown className="w-4 h-4" />
    )
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm text-left text-ink-muted">
        <thead className="text-xs text-ink uppercase bg-surface-2">
          <tr>
            {dragEnabled && (
              <th scope="col" className="pl-2 w-6">
                {/* Drag handle column */}
              </th>
            )}
            <th scope="col" className="px-2 py-1 w-16">
              {/* Icon column - no sort */}
            </th>
            <th
              scope="col"
              className="px-2 py-1 cursor-pointer hover:bg-raised"
              onClick={() => onSort('name')}
            >
              <div className="flex items-center gap-1">
                Name
                <SortIcon columnKey="name" />
              </div>
            </th>
            <th
              scope="col"
              className="px-2 py-1 cursor-pointer hover:bg-raised"
              onClick={() => onSort('description')}
            >
              <div className="flex items-center gap-1">
                Description
                <SortIcon columnKey="description" />
              </div>
            </th>
            <th
              scope="col"
              className="px-2 py-1 cursor-pointer hover:bg-raised"
              onClick={() => onSort('tags')}
            >
              <div className="flex items-center gap-1">
                Tags
                <SortIcon columnKey="tags" />
              </div>
            </th>
            <th
              scope="col"
              className="px-2 py-1 cursor-pointer hover:bg-raised"
              onClick={() => onSort('status')}
            >
              <div className="flex items-center gap-1">
                {t('status')}
                <SortIcon columnKey="status" />
              </div>
            </th>
            <th scope="col" className="px-2 py-1 w-24">
              Actions
            </th>
          </tr>
        </thead>
        <tbody>
          {agents.map((agent) => {
            const isCustomAgent = agent.isCustom ?? true
            const isSelected = agent.id === selectedAgentId

            return (
              <tr
                key={agent.id}
                className={`border-b border-subtle cursor-pointer
                  bg-surface hover:bg-surface-2
                  ${dragClassName?.(agent.id) || ''}`}
                onClick={() => onSelectAgent(agent.id!)}
                {...dragProps?.(agent.id)}
              >
                {dragEnabled && (
                  <td className="pl-2 w-6 text-ink-faint">
                    <MdDragIndicator className="w-4 h-4" title={t('myAgents.dragToReorder')} />
                  </td>
                )}
                {/* Icon */}
                <td className="px-2 py-1">
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
                </td>

                {/* Name */}
                <td className="px-2 py-1 font-medium text-ink">{agent.name}</td>

                {/* Description */}
                <td className="px-2 py-1">
                  <div className="line-clamp-2 text-ink-muted">
                    {t(agent.description) || t('noDescription')}
                  </div>
                </td>

                {/* Tags */}
                <td className="px-2 py-1">
                  <div className="flex flex-wrap gap-1">
                    {agent.tags?.slice(0, 3).map((tag) => (
                      <span
                        key={tag}
                        className="inline-flex items-center px-2 py-0.5 rounded-control text-xs font-medium
                          bg-accent-tint text-accent"
                      >
                        {tag}
                      </span>
                    ))}
                    {agent.tags && agent.tags.length > 3 && (
                      <span className="text-xs text-ink-muted">+{agent.tags.length - 3}</span>
                    )}
                  </div>
                </td>

                {/* Status */}
                <td className="px-2 py-1">
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
                </td>

                {/* Actions */}
                <td className="px-2 py-1">
                  <AgentActionsDropdown
                    agent={agent}
                    onEdit={onEditAgent}
                    onDuplicate={onDuplicateAgent}
                    onDelete={onDeleteAgent}
                    onSaveAsShared={onSaveAsShared}
                    onDeleteSharedFile={onDeleteSharedFile}
                    onDownloadYaml={onDownloadYaml}
                    onShareToOrganization={onShareToOrganization}
                    onConvertToStrands={onConvertToStrands}
                  />
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

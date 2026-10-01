import React, { useState, useEffect } from 'react'
import { FiSearch, FiUpload } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import { CustomAgent } from '@/types/agent-chat'
import { AgentCard } from './AgentCard'
import { AgentTableView } from './AgentTableView'
import { AgentViewToggle } from './AgentViewToggle'
import { EmptyState } from './EmptyState'
import { TagFilter } from './TagFilter'
import { UnhideAgentsDropdown } from './UnhideAgentsDropdown'
import { useAgentFilter } from './useAgentFilter'
import { useAgentDragOrder } from './useAgentDragOrder'
import { useSettings } from '@renderer/contexts/SettingsContext'

interface AgentListProps {
  agents: CustomAgent[]
  selectedAgentId?: string
  onSelectAgent: (agentId: string) => void
  onAddNewAgent: () => void
  /** Bring in an agent from a YAML/JSON file as a new editable agent */
  onImportAgent?: () => void
  onEditAgent: (agent: CustomAgent) => void
  onDuplicateAgent: (agent: CustomAgent) => void
  onDeleteAgent: (agentId: string) => void
  onSaveAsShared?: (agent: CustomAgent) => void
  onDeleteSharedFile?: (agent: CustomAgent) => void
  onDownloadYaml?: (agent: CustomAgent) => void
  onShareToOrganization?: (agent: CustomAgent) => void
  onConvertToStrands?: (agentId: string) => void
  /** Default agents the user hid; drives the unhide dropdown */
  hiddenAgents?: CustomAgent[]
  onUnhideAgent?: (agentId: string) => void
  onUnhideAll?: () => void
  /** Allow rearranging agents by drag & drop (My Agents page) */
  allowReorder?: boolean
}

export const AgentList: React.FC<AgentListProps> = ({
  agents,
  selectedAgentId,
  onSelectAgent,
  onAddNewAgent,
  onImportAgent,
  onEditAgent,
  onDuplicateAgent,
  onDeleteAgent,
  onSaveAsShared,
  onDeleteSharedFile,
  onDownloadYaml,
  onShareToOrganization,
  onConvertToStrands,
  hiddenAgents,
  onUnhideAgent,
  onUnhideAll,
  allowReorder = false
}) => {
  const { t } = useTranslation()
  const { agentListViewMode, setAgentListViewMode } = useSettings()
  const [viewMode, setViewMode] = useState<'card' | 'table'>(agentListViewMode)
  const {
    searchQuery,
    setSearchQuery,
    selectedTags,
    availableTags,
    filteredAgents,
    toggleTag,
    sortKey,
    sortOrder,
    handleSort
  } = useAgentFilter(agents)

  // Sync viewMode with SettingsContext
  useEffect(() => {
    setAgentListViewMode(viewMode)
  }, [viewMode, setAgentListViewMode])

  // A column sort would fight the manual arrangement, so dragging is off then
  const isDragEnabled = allowReorder && sortKey === null
  const dragOrder = useAgentDragOrder(agents, isDragEnabled)

  return (
    <div className="p-2.5 bg-surface">
      <div className="flex items-center justify-between gap-4 mb-4">
        <div className="relative flex-1 max-w-md">
          <div className="absolute inset-y-0 left-0 flex items-center pl-3 pointer-events-none">
            <FiSearch className="w-4 h-4 text-ink-faint" />
          </div>
          <input
            type="search"
            className="block w-full p-2 pl-10 text-sm text-ink border border-strong rounded-container
              bg-surface-2 focus:ring-accent focus:border-accent
              border-subtle placeholder-ink-faint text-ink
              focus:ring-accent focus:border-accent"
            placeholder={t('searchAgents')}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
        <div className="flex items-center gap-2">
          {hiddenAgents && onUnhideAgent && onUnhideAll && (
            <UnhideAgentsDropdown
              hiddenAgents={hiddenAgents}
              onUnhideAgent={onUnhideAgent}
              onUnhideAll={onUnhideAll}
            />
          )}
          <AgentViewToggle viewMode={viewMode} onToggle={setViewMode} />
          {onImportAgent && (
            <button
              onClick={onImportAgent}
              className="px-2.5 py-1 text-sm font-medium text-ink
                bg-surface border border-strong rounded-container
                shadow-sm hover:bg-surface-2 focus:outline-none focus:ring-2
                focus:ring-offset-2 focus:ring-accent focus:ring-offset-canvas
                whitespace-nowrap flex gap-2 items-center"
            >
              <FiUpload className="w-4 h-4" />
              {t('importAgent')}
            </button>
          )}
          <button
            onClick={onAddNewAgent}
            className="px-2.5 py-1 text-sm font-medium text-accent-fg bg-accent
              border border-transparent rounded-container shadow-sm hover:bg-accent-strong
              focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent
              focus:ring-offset-canvas whitespace-nowrap flex gap-2 items-center"
          >
            {t('addNewAgent')}
          </button>
        </div>
      </div>

      <TagFilter tags={availableTags} selectedTags={selectedTags} onSelectTag={toggleTag} />

      {filteredAgents.length === 0 ? (
        <EmptyState />
      ) : viewMode === 'card' ? (
        <div className="grid gap-4 lg:grid-cols-2 xl:grid-cols-3">
          {filteredAgents.map((agent) => {
            const isCustomAgent = agent.isCustom ?? true
            const isSelected = agent.id === selectedAgentId
            // Shared agents can't be edited or deleted
            const isEditable = isCustomAgent && !agent.isShared

            return (
              <AgentCard
                key={agent.id}
                agent={agent as CustomAgent}
                isCustomAgent={isCustomAgent}
                isSelected={isSelected}
                onSelect={onSelectAgent}
                onEdit={isEditable ? onEditAgent : undefined}
                onDuplicate={onDuplicateAgent}
                onDelete={onDeleteAgent}
                onSaveAsShared={onSaveAsShared}
                onDeleteSharedFile={onDeleteSharedFile}
                onDownloadYaml={onDownloadYaml}
                onShareToOrganization={isEditable ? onShareToOrganization : undefined}
                onConvertToStrands={onConvertToStrands}
                dragProps={dragOrder.dragProps(agent.id)}
                dragClassName={dragOrder.dragClassName(agent.id)}
              />
            )
          })}
        </div>
      ) : (
        <AgentTableView
          agents={filteredAgents}
          selectedAgentId={selectedAgentId}
          onSelectAgent={onSelectAgent}
          onEditAgent={onEditAgent}
          onDuplicateAgent={onDuplicateAgent}
          onDeleteAgent={onDeleteAgent}
          onSaveAsShared={onSaveAsShared}
          onDeleteSharedFile={onDeleteSharedFile}
          onDownloadYaml={onDownloadYaml}
          onShareToOrganization={onShareToOrganization}
          onConvertToStrands={onConvertToStrands}
          sortKey={sortKey}
          sortOrder={sortOrder}
          onSort={handleSort}
          dragProps={dragOrder.dragProps}
          dragClassName={dragOrder.dragClassName}
          dragEnabled={isDragEnabled}
        />
      )}
    </div>
  )
}

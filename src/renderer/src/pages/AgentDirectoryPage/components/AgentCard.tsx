import React from 'react'
import { useTranslation } from 'react-i18next'
import { TbRobot } from 'react-icons/tb'
import { CustomAgent } from '@/types/agent-chat'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'

interface AgentCardProps {
  agent: CustomAgent
  onSelect: (agent: CustomAgent) => void
  onTagClick?: (tag: string) => void
}

export const AgentCard: React.FC<AgentCardProps> = ({ agent, onSelect, onTagClick }) => {
  const { t } = useTranslation()

  return (
    <div
      className="bg-surface rounded-container shadow-sm border border-subtle p-5 transition-all hover:shadow-md cursor-pointer"
      onClick={() => onSelect(agent)}
    >
      <div className="flex items-start">
        <div className="w-10 h-10 rounded-full flex items-center justify-center mr-3 flex-shrink-0 bg-raised">
          {agent.icon ? (
            <AgentIconView icon={agent.icon} iconColor={agent.iconColor || '#3B82F6'} />
          ) : (
            <TbRobot className="w-4 h-4" style={{ color: agent.iconColor || '#3B82F6' }} />
          )}
        </div>
        <div>
          <h3 className="font-medium text-heading text-ink">{agent.name}</h3>
          <p className="text-ink-muted text-sm line-clamp-2">{agent.description}</p>
        </div>
      </div>

      {/* Tags */}
      {agent.tags && agent.tags.length > 0 && (
        <div className="flex flex-wrap gap-2 mt-4">
          {agent.tags.slice(0, 3).map((tag) => (
            <button
              key={tag}
              className="bg-raised text-ink text-xs font-medium px-2.5 py-0.5 rounded-control hover:bg-sunken transition-colors"
              onClick={(e) => {
                e.stopPropagation() // Prevent card click from triggering
                if (onTagClick) onTagClick(tag)
              }}
            >
              {tag}
            </button>
          ))}
          {agent.tags.length > 3 && (
            <span className="bg-raised text-ink text-xs font-medium px-2.5 py-0.5 rounded-control">
              +{agent.tags.length - 3}
            </span>
          )}
        </div>
      )}

      {/* Author */}
      {agent.author && (
        <div className="mt-4 text-xs text-ink-muted">
          {t('authorLabel')}: {agent.author}
        </div>
      )}
    </div>
  )
}

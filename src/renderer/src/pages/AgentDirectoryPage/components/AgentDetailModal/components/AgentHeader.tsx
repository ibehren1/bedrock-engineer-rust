import React from 'react'
import { TbRobot } from 'react-icons/tb'
import { AgentIcon } from '@/types/agent-chat'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'

interface AgentHeaderProps {
  name: string
  description?: string
  icon?: AgentIcon
  iconColor?: string
}

export const AgentHeader: React.FC<AgentHeaderProps> = ({ name, description, icon, iconColor }) => {
  const defaultColor = '#3B82F6'

  return (
    <div className="flex items-start">
      <div className="w-8 h-8 rounded-full flex items-center justify-center mr-2 bg-raised flex-shrink-0">
        {icon ? (
          <AgentIconView
            icon={icon}
            iconColor={iconColor || defaultColor}
            fallback={<TbRobot className="w-4 h-4" style={{ color: iconColor || defaultColor }} />}
          />
        ) : (
          <TbRobot className="w-4 h-4" style={{ color: iconColor || defaultColor }} />
        )}
      </div>
      <div>
        <h2 className="text-title text-ink">{name}</h2>
        {description && <p className="text-ink-muted">{description}</p>}
      </div>
    </div>
  )
}

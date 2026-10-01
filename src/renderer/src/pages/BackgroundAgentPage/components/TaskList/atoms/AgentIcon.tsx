import React from 'react'
import { CogIcon } from '@heroicons/react/24/outline'
import { AgentIconView } from '@renderer/components/icons/AgentIconView'

interface AgentIconProps {
  agent: { icon?: string; iconColor?: string; name: string } | null
  size?: 'sm' | 'md'
}

export const AgentIcon: React.FC<AgentIconProps> = ({ agent, size = 'sm' }) => {
  const iconSize = size === 'sm' ? 'h-4 w-4' : 'h-4 w-4'

  if (!agent?.icon) {
    return <CogIcon className={`${iconSize} flex-shrink-0`} />
  }

  return (
    <div className={`${iconSize} flex items-center justify-center flex-shrink-0`}>
      <AgentIconView icon={agent.icon} iconColor={agent.iconColor} />
    </div>
  )
}

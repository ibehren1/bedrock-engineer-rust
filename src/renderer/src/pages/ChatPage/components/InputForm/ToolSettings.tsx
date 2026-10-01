import React from 'react'
import { FcSupport } from 'react-icons/fc'

type ToolSettingsProps = {
  onOpenToolSettings: () => void
}

export const ToolSettings: React.FC<ToolSettingsProps> = ({ onOpenToolSettings }) => {
  return (
    <label
      onClick={onOpenToolSettings}
      className="block text-sm font-medium text-ink cursor-pointer hover:text-ink-muted"
    >
      <div className="flex gap-2 items-center">
        <FcSupport className="text-base" />
        <span>Tools</span>
      </div>
    </label>
  )
}

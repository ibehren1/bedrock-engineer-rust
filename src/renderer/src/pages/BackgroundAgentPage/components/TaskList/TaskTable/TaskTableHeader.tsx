import React from 'react'

interface TaskTableHeaderProps {
  // i18n labels
  nameLabel: string
  scheduleLabel: string
  agentLabel: string
  statusLabel: string
  lastRunLabel: string
  actionsLabel: string
}

export const TaskTableHeader: React.FC<TaskTableHeaderProps> = ({
  nameLabel,
  scheduleLabel,
  agentLabel,
  statusLabel,
  lastRunLabel,
  actionsLabel
}) => {
  return (
    <thead className="bg-surface-2">
      <tr>
        <th className="px-2 py-1 text-left text-micro text-ink-muted uppercase">{nameLabel}</th>
        <th className="px-2 py-1 text-left text-micro text-ink-muted uppercase">{scheduleLabel}</th>
        <th className="px-2 py-1 text-left text-micro text-ink-muted uppercase">{agentLabel}</th>
        <th className="px-2 py-1 text-left text-micro text-ink-muted uppercase">{statusLabel}</th>
        <th className="px-2 py-1 text-left text-micro text-ink-muted uppercase">{lastRunLabel}</th>
        <th className="px-2 py-1 text-right text-micro text-ink-muted uppercase">{actionsLabel}</th>
      </tr>
    </thead>
  )
}

import React, { useMemo, useState } from 'react'
import { Modal, ModalBody, ModalFooter, ModalHeader } from 'flowbite-react'
import { useTranslation } from 'react-i18next'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { Button, Input } from '@renderer/components/ui'
import type { CommandConfig } from '@/types/agent-chat'
import { SettingSection } from '../SettingSection'

/**
 * Every agent's Allowed Commands in one place (the same lists the agent editor's Command tool
 * settings edit). Agents shared through a project's `.bedrock-engineer/agents/` are shown
 * read-only: their lists come from the file.
 */
export const AllowedCommandsSection: React.FC = () => {
  const { t } = useTranslation()
  const { customAgents, sharedAgents } = useSettings()
  const [open, setOpen] = useState(false)
  const total = useMemo(
    () =>
      [...customAgents, ...sharedAgents].reduce(
        (sum, agent) => sum + (agent.allowedCommands?.length ?? 0),
        0
      ),
    [customAgents, sharedAgents]
  )

  return (
    <SettingSection
      title={t('allowedCommandsSettings.title')}
      description={t('allowedCommandsSettings.description')}
    >
      <div className="flex items-center gap-3">
        <span className="text-sm text-ink-muted">
          {t('allowedCommandsSettings.summary', { count: total })}
        </span>
        <Button size="sm" onClick={() => setOpen(true)}>
          {t('allowedCommandsSettings.manage')}
        </Button>
      </div>
      {open && <AllowedCommandsModal onClose={() => setOpen(false)} />}
    </SettingSection>
  )
}

const AllowedCommandsModal: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation()
  const { customAgents, sharedAgents, updateAgentAllowedCommands } = useSettings()

  return (
    <Modal dismissible show onClose={onClose} size="4xl">
      <ModalHeader>{t('allowedCommandsSettings.title')}</ModalHeader>
      <ModalBody>
        <div className="flex flex-col gap-4">
          <p className="text-sm text-ink-muted">{t('allowedCommandsSettings.modalHelp')}</p>
          {customAgents.map((agent) => (
            <AgentCommands
              key={agent.id}
              name={agent.name}
              commands={agent.allowedCommands ?? []}
              onChange={(commands) => updateAgentAllowedCommands(agent.id!, commands)}
            />
          ))}
          {sharedAgents.map((agent) => (
            <AgentCommands
              key={agent.id}
              name={agent.name}
              commands={agent.allowedCommands ?? []}
              readOnlyNote={t('allowedCommandsSettings.sharedReadOnly')}
            />
          ))}
        </div>
      </ModalBody>
      <ModalFooter>
        <Button variant="primary" onClick={onClose}>
          {t('allowedCommandsSettings.done')}
        </Button>
      </ModalFooter>
    </Modal>
  )
}

const AgentCommands: React.FC<{
  name: string
  commands: CommandConfig[]
  onChange?: (commands: CommandConfig[]) => void
  readOnlyNote?: string
}> = ({ name, commands, onChange, readOnlyNote }) => {
  const { t } = useTranslation()
  const [pattern, setPattern] = useState('')
  const [description, setDescription] = useState('')
  const trimmed = pattern.trim()
  const duplicate = commands.some((c) => c.pattern === trimmed)

  const add = () => {
    if (!onChange || !trimmed || duplicate) return
    onChange([...commands, { pattern: trimmed, description: description.trim() }])
    setPattern('')
    setDescription('')
  }

  return (
    <section className="rounded-control border border-subtle p-3">
      <h3 className="m-0 mb-2 text-sm font-semibold text-ink">{name}</h3>
      {readOnlyNote && <p className="mb-2 text-xs text-ink-muted">{readOnlyNote}</p>}
      {commands.length === 0 ? (
        <p className="text-xs text-ink-muted">{t('allowedCommandsSettings.none')}</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {commands.map((command) => (
            <li key={command.pattern} className="flex items-center gap-2 text-sm text-ink">
              <code className="rounded-sm bg-raised px-1.5 py-0.5">{command.pattern}</code>
              {command.description && (
                <span className="text-xs text-ink-muted">{command.description}</span>
              )}
              {onChange && (
                <Button
                  size="sm"
                  variant="ghost"
                  className="ml-auto"
                  onClick={() => onChange(commands.filter((c) => c.pattern !== command.pattern))}
                >
                  {t('allowedCommandsSettings.remove')}
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
      {onChange && (
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Input
            className="w-56"
            value={pattern}
            placeholder={t('allowedCommandsSettings.patternPlaceholder')}
            onChange={(e) => setPattern(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && add()}
          />
          <Input
            className="flex-1 min-w-48"
            value={description}
            placeholder={t('allowedCommandsSettings.descriptionPlaceholder')}
            onChange={(e) => setDescription(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && add()}
          />
          <Button size="sm" onClick={add} disabled={!trimmed || duplicate}>
            {t('allowedCommandsSettings.add')}
          </Button>
        </div>
      )}
    </section>
  )
}

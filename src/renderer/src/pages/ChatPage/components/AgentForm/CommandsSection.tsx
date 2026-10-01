import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { CommandConfig } from '@/types/agent-chat'
import { EditIcon, RemoveIcon } from '@renderer/components/icons/ToolIcons'
import { Button, Input, Label, Textarea } from '@renderer/components/ui'

interface CommandsSectionProps {
  commands: CommandConfig[]
  onChange: (commands: CommandConfig[]) => void
}

export const CommandsSection: React.FC<CommandsSectionProps> = ({ commands = [], onChange }) => {
  const { t } = useTranslation()
  const [newCommand, setNewCommand] = useState('')
  const [newDescription, setNewDescription] = useState('')
  const [editMode, setEditMode] = useState<string | null>(null)
  const [editData, setEditData] = useState<CommandConfig>({ pattern: '', description: '' })

  const handleAddCommand = () => {
    if (newCommand.trim() && newDescription.trim()) {
      onChange([
        ...commands,
        {
          pattern: newCommand.trim(),
          description: newDescription.trim()
        }
      ])
      setNewCommand('')
      setNewDescription('')
    }
  }

  const handleRemoveCommand = (pattern: string) => {
    onChange(commands.filter((cmd) => cmd.pattern !== pattern))
  }

  const handleEditCommand = (command: CommandConfig) => {
    setEditMode(command.pattern)
    setEditData({ ...command })
  }

  const handleSaveEdit = () => {
    if (editData.pattern.trim() && editData.description.trim()) {
      onChange(commands.map((cmd) => (cmd.pattern === editMode ? { ...editData } : cmd)))
      setEditMode(null)
      setEditData({ pattern: '', description: '' })
    }
  }

  const handleCancelEdit = () => {
    setEditMode(null)
    setEditData({ pattern: '', description: '' })
  }

  return (
    <div className="space-y-2 border border-subtle rounded-control p-2.5">
      <div className="flex justify-between items-center">
        <h3 className="text-heading font-semibold text-ink">{t('Allowed Commands')}</h3>
      </div>

      <p className="text-sm text-ink-muted">
        {t('Configure which system commands the agent is allowed to execute.')}
      </p>

      <div className="bg-warning-soft p-3 rounded-control">
        <h5 className="font-medium mb-1 text-warning text-sm">{t('Security Warning')}</h5>
        <p className="text-xs text-ink">
          {t(
            'Only allow commands that you trust this agent to execute. Use wildcards (*) to define patterns.'
          )}
        </p>
      </div>

      {/* コマンド追加フォーム */}
      <div className="flex flex-col gap-2 mt-4">
        <h4 className="font-medium text-sm mb-2">{t('Add New Command Pattern')}</h4>
        <div>
          <Label>{t('Command Pattern')}</Label>
          <Input
            type="text"
            value={newCommand}
            onChange={(e) => setNewCommand(e.target.value)}
            placeholder="e.g., ls *"
          />
          <p className="text-xs text-ink-muted mt-1">
            {t('Use * as a wildcard (e.g., "npm *" allows all npm commands)')}
          </p>
        </div>
        <div className="mt-2">
          <Label>{t('Description')}</Label>
          <Textarea
            value={newDescription}
            onChange={(e) => setNewDescription(e.target.value)}
            placeholder="e.g., List directory contents"
            rows={2}
          />
        </div>
        <Button
          onClick={handleAddCommand}
          disabled={!newCommand.trim() || !newDescription.trim()}
          variant="primary"
          className="mt-2 w-fit"
        >
          {t('Add Command')}
        </Button>
      </div>

      {/* 登録済みコマンドリスト */}
      <div className="space-y-3 mt-3">
        <h4 className="font-medium text-sm">{t('Current Command Patterns')}</h4>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
          {commands.length === 0 ? (
            <p className="text-sm text-ink-muted italic">
              {t('No command patterns registered yet')}
            </p>
          ) : (
            commands.map((command) => (
              <div
                key={command.pattern}
                className="flex flex-col p-3 text-sm bg-canvas text-ink rounded-control border border-subtle"
              >
                {editMode === command.pattern ? (
                  // 編集モード
                  <div className="flex flex-col gap-2">
                    <div>
                      <Label>{t('Command Pattern')}</Label>
                      <Input
                        type="text"
                        value={editData.pattern}
                        onChange={(e) => setEditData({ ...editData, pattern: e.target.value })}
                      />
                    </div>
                    <div className="mt-2">
                      <Label>{t('Description')}</Label>
                      <Textarea
                        value={editData.description}
                        onChange={(e) => setEditData({ ...editData, description: e.target.value })}
                        rows={2}
                      />
                    </div>
                    <div className="flex justify-end gap-2 mt-2">
                      <button
                        onClick={handleCancelEdit}
                        className="px-3 py-1 text-sm text-ink-muted border border-strong rounded-control hover:bg-raised"
                      >
                        {t('Cancel')}
                      </button>
                      <Button
                        onClick={handleSaveEdit}
                        disabled={!editData.pattern.trim() || !editData.description.trim()}
                        variant="primary"
                      >
                        {t('Save')}
                      </Button>
                    </div>
                  </div>
                ) : (
                  // 表示モード
                  <>
                    <div className="flex items-center justify-between">
                      <span className="font-mono font-medium">{command.pattern}</span>
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleEditCommand(command)}
                          className="text-accent hover:text-accent p-1"
                          title="Edit"
                          aria-label="Edit command"
                        >
                          <EditIcon />
                        </button>
                        <button
                          onClick={() => handleRemoveCommand(command.pattern)}
                          className="text-danger hover:text-danger-strong p-1"
                          title="Remove"
                          aria-label="Remove command"
                        >
                          <RemoveIcon />
                        </button>
                      </div>
                    </div>
                    <p className="text-xs text-ink-muted mt-1 whitespace-pre-line">
                      {command.description}
                    </p>
                  </>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}

export default CommandsSection

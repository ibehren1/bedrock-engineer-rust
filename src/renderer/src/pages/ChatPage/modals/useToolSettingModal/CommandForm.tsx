/* eslint-disable react/no-unescaped-entities */
import { memo, useState, useMemo } from 'react'
import { CommandConfig, AVAILABLE_SHELLS } from '.'
import { EditIcon, RemoveIcon } from '@renderer/components/icons/ToolIcons'
import { useTranslation } from 'react-i18next'
import { Button, Input, Label, Select, Textarea } from '@renderer/components/ui'

// コマンド設定フォームコンポーネント
export const CommandForm = memo(
  ({
    allowedCommands,
    setAllowedCommands,
    shell,
    setShell
  }: {
    allowedCommands: CommandConfig[]
    setAllowedCommands: (commands: CommandConfig[]) => void
    shell: string
    setShell: (shell: string) => void
  }) => {
    const { t } = useTranslation()
    const [newCommand, setNewCommand] = useState('')
    const [newDescription, setNewDescription] = useState('')
    const [editMode, setEditMode] = useState<string | null>(null)
    const [editData, setEditData] = useState<CommandConfig>({ pattern: '', description: '' })

    // Windows環境かどうかを判定
    const isWindows = useMemo(() => {
      return navigator.platform.toLowerCase().includes('win')
    }, [])

    const handleAddCommand = () => {
      if (newCommand.trim() && newDescription.trim()) {
        setAllowedCommands([
          ...allowedCommands,
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
      setAllowedCommands(allowedCommands.filter((cmd) => cmd.pattern !== pattern))
    }

    const handleEditCommand = (command: CommandConfig) => {
      setEditMode(command.pattern)
      setEditData({ ...command })
    }

    const handleSaveEdit = () => {
      if (editData.pattern.trim() && editData.description.trim()) {
        setAllowedCommands(
          allowedCommands.map((cmd) => (cmd.pattern === editMode ? { ...editData } : cmd))
        )
        setEditMode(null)
        setEditData({ pattern: '', description: '' })
      }
    }

    const handleCancelEdit = () => {
      setEditMode(null)
      setEditData({ pattern: '', description: '' })
    }

    return (
      <div className="mt-4 space-y-2">
        {/* ツールの説明 */}
        <div className="max-w-none">
          <p className="mb-4 text-ink">{t('tool info.executeCommand.description')}</p>

          <div className="bg-warning-soft p-2.5 rounded-control mb-5">
            <h5 className="font-medium mb-2 text-warning">
              {t('tool info.executeCommand.warning title')}
            </h5>
            <p className="text-sm text-ink">{t('tool info.executeCommand.warning description')}</p>
          </div>

          <div className="bg-accent-tint p-2.5 rounded-control mb-5">
            <h5 className="font-medium mb-2 text-ink">
              {t('tool info.executeCommand.example title')}
            </h5>
            <p className="text-sm text-ink">{t('tool info.executeCommand.example description')}</p>
          </div>
        </div>

        {/* シェル選択 */}
        <div className="space-y-2 p-2.5 border border-subtle rounded-control">
          <h4 className="font-medium text-sm mb-3 text-ink">{t('Command Shell Settings')}</h4>
          <Label>{t('Command Shell')}</Label>
          <Select value={shell} onChange={(e) => setShell(e.target.value)}>
            {AVAILABLE_SHELLS.map((shellOption) => (
              <option key={shellOption.value} value={shellOption.value}>
                {shellOption.label}
              </option>
            ))}
          </Select>
          <div className="mt-2 space-y-2">
            <p className="text-xs text-ink-muted">
              {t('Select which shell to use when executing commands')}
            </p>

            {/* Windows環境での注意事項 */}
            {isWindows && (
              <div className="bg-success-soft p-3 rounded-control">
                <div className="flex items-start">
                  <div className="flex-shrink-0">
                    <svg
                      className="h-4 w-4 text-success mt-0.5"
                      fill="currentColor"
                      viewBox="0 0 20 20"
                    >
                      <path
                        fillRule="evenodd"
                        d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                        clipRule="evenodd"
                      />
                    </svg>
                  </div>
                  <div className="ml-2">
                    <h5 className="text-xs font-medium text-success">
                      {t('Windows Environment Notice')}
                    </h5>
                    <p className="text-xs text-success mt-1">{t('Windows shell execution note')}</p>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* コマンド追加フォーム */}
        <div className="flex flex-col gap-2 p-2.5 border border-subtle rounded-control mt-4">
          <h4 className="font-medium text-sm mb-2 text-ink">{t('Add New Command Pattern')}</h4>
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
              rows={3}
            />
          </div>
          <Button
            onClick={handleAddCommand}
            disabled={!newCommand.trim() || !newDescription.trim()}
            variant="primary"
            className="mt-2"
          >
            {t('Add Command')}
          </Button>
        </div>

        {/* 登録済みコマンドリスト */}
        <div className="space-y-3 mt-3">
          <h4 className="font-medium text-sm text-ink">{t('Allowed Command Patterns')}</h4>
          <div className=" grid grid-cols-2 gap-2">
            {allowedCommands.length === 0 ? (
              <p className="text-sm text-ink-muted italic">
                {t('No command patterns registered yet')}
              </p>
            ) : (
              allowedCommands.map((command) => (
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
                          onChange={(e) =>
                            setEditData({ ...editData, description: e.target.value })
                          }
                          rows={3}
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
)
CommandForm.displayName = 'CommandForm'

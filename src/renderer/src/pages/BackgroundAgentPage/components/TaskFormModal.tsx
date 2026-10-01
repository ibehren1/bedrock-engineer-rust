import React, { useState, useEffect, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { XMarkIcon } from '@heroicons/react/24/outline'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { ModelSelector } from '../../ChatPage/components/ModelSelector'
import { DirectorySelector } from '../../ChatPage/components/InputForm/DirectorySelector'
import { AgentSelector } from '../../ChatPage/components/AgentSelector'
import { IgnoreSettingsModal } from '@renderer/components/IgnoreSettingsModal'
import { ScheduleConfig, ScheduledTask } from '../hooks/useBackgroundAgent'

// Fallback ceiling for the Max Output Tokens field, used until the selected
// model's own limit has been looked up (or if that lookup fails). Matches the
// default in getModelMaxTokens.
const DEFAULT_MODEL_MAX_TOKENS = 8192

interface TaskFormModalProps {
  mode: 'create' | 'edit'
  task?: ScheduledTask // 編集時のみ
  onSubmit: (config: ScheduleConfig, taskId?: string) => Promise<void>
  onCancel: () => void
}

export const TaskFormModal: React.FC<TaskFormModalProps> = ({ mode, task, onSubmit, onCancel }) => {
  const { t } = useTranslation()
  const { agents, availableModels } = useSettings()

  const isEditMode = mode === 'edit'

  const CRON_PRESETS = [
    { label: t('backgroundAgent.cronPresets.everyMinute'), value: '* * * * *' },
    { label: t('backgroundAgent.cronPresets.every5Minutes'), value: '*/5 * * * *' },
    { label: t('backgroundAgent.cronPresets.everyHour'), value: '0 * * * *' },
    { label: t('backgroundAgent.cronPresets.dailyAt9AM'), value: '0 9 * * *' },
    { label: t('backgroundAgent.cronPresets.weekdaysAt9AM'), value: '0 9 * * 1-5' },
    { label: t('backgroundAgent.cronPresets.weeklyMondayAt9AM'), value: '0 9 * * 1' },
    { label: t('backgroundAgent.cronPresets.monthlyFirst9AM'), value: '0 9 1 * *' }
  ]

  const getInitialFormData = useCallback(() => {
    if (isEditMode && task) {
      return {
        name: task.name,
        cronExpression: task.cronExpression,
        agentId: task.agentId,
        modelId: task.modelId,
        projectDirectory: task.projectDirectory || '',
        wakeWord: task.wakeWord,
        enabled: task.enabled,
        maxTokens: task.inferenceConfig?.maxTokens || 4096,
        continueSession: task.continueSession || false,
        continueSessionPrompt: task.continueSessionPrompt || ''
      }
    }
    return {
      name: '',
      cronExpression: '0 9 * * 1-5', // Default: Weekdays at 9 AM
      agentId: '',
      modelId: '', // 動的に設定
      projectDirectory: '',
      wakeWord: '',
      enabled: true,
      maxTokens: 4096, // Default max tokens
      continueSession: false, // セッション継続フラグ
      continueSessionPrompt: '' // セッション継続時専用プロンプト
    }
  }, [isEditMode, task])

  const [formData, setFormData] = useState(getInitialFormData)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [showProjectIgnoreModal, setShowProjectIgnoreModal] = useState(false)
  // Output ceiling of the currently selected model, used as the upper bound for
  // the Max Output Tokens field instead of one hardcoded number for all models.
  const [modelMaxTokens, setModelMaxTokens] = useState(DEFAULT_MODEL_MAX_TOKENS)

  // プロジェクトディレクトリ選択ハンドラー
  const handleSelectDirectory = async () => {
    try {
      const selectedPath = await window.api.openDirectory()
      if (selectedPath) {
        setFormData((prev) => ({ ...prev, projectDirectory: selectedPath }))
      }
    } catch (error) {
      console.error('Failed to select directory:', error)
    }
  }

  // プロジェクト固有の.ignoreモーダルを開く
  const handleOpenIgnoreModal = () => {
    if (formData.projectDirectory) {
      setShowProjectIgnoreModal(true)
    }
  }

  // デフォルトのエージェントを設定（新規作成時のみ）
  useEffect(() => {
    if (!isEditMode && agents.length > 0 && !formData.agentId) {
      setFormData((prev) => ({ ...prev, agentId: agents[0].id }))
    }
  }, [isEditMode, agents, formData.agentId])

  // デフォルトのモデルを設定（新規作成時のみ）
  useEffect(() => {
    if (!isEditMode && availableModels.length > 0 && !formData.modelId) {
      setFormData((prev) => ({ ...prev, modelId: availableModels[0].modelId }))
    }
  }, [isEditMode, availableModels, formData.modelId])

  // モデル変更時にmaxTokensの制限を調整
  useEffect(() => {
    const updateMaxTokens = async () => {
      let maxTokensLimit = DEFAULT_MODEL_MAX_TOKENS
      try {
        const result = await window.api.bedrock.getModelMaxTokens(formData.modelId)
        maxTokensLimit = result.maxTokens
      } catch (error) {
        console.error('Failed to get model max tokens:', error)
        // エラーの場合はデフォルト値を使用
      }

      setModelMaxTokens(maxTokensLimit)
      if (formData.maxTokens > maxTokensLimit) {
        setFormData((prev) => ({ ...prev, maxTokens: maxTokensLimit }))
      }
    }

    updateMaxTokens()
  }, [formData.modelId, formData.maxTokens])

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {}

    if (!formData.name.trim()) {
      newErrors.name = t('backgroundAgent.form.errors.nameRequired')
    }

    if (!formData.cronExpression.trim()) {
      newErrors.cronExpression = t('backgroundAgent.form.errors.cronRequired')
    }

    if (!formData.agentId) {
      newErrors.agentId = t('backgroundAgent.form.errors.agentRequired')
    }

    // モデル選択の詳細バリデーション
    if (!formData.modelId) {
      newErrors.modelId = t('backgroundAgent.form.errors.modelRequired')
    } else if (!isEditMode) {
      // 新規作成時のみ詳細バリデーション
      // 利用可能なモデルリストが空の場合
      if (availableModels.length === 0) {
        newErrors.modelId = 'Available models are not loaded. Please check your AWS configuration.'
      } else {
        // 選択されたモデルが利用可能なモデルリストに存在するかチェック
        const modelExists = availableModels.some((model) => model.modelId === formData.modelId)
        if (!modelExists) {
          newErrors.modelId = `Selected model "${formData.modelId}" is not available. Please choose from the available models.`
        }
      }
    }

    if (!formData.wakeWord.trim()) {
      newErrors.wakeWord = t('backgroundAgent.form.errors.wakeWordRequired')
    }

    if (formData.maxTokens < 1 || formData.maxTokens > modelMaxTokens) {
      newErrors.maxTokens = t('backgroundAgent.form.errors.invalidMaxTokens', {
        max: modelMaxTokens
      })
    }

    setErrors(newErrors)
    return Object.keys(newErrors).length === 0
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    if (!validateForm()) {
      return
    }

    setIsSubmitting(true)

    try {
      const config: ScheduleConfig = {
        name: formData.name,
        cronExpression: formData.cronExpression,
        agentConfig: {
          modelId: formData.modelId,
          agentId: formData.agentId,
          projectDirectory: formData.projectDirectory || undefined,
          inferenceConfig: {
            maxTokens: formData.maxTokens
          }
        },
        wakeWord: formData.wakeWord,
        enabled: formData.enabled,
        continueSession: formData.continueSession,
        continueSessionPrompt: formData.continueSessionPrompt || undefined
      }

      await onSubmit(config, isEditMode && task ? task.id : undefined)
    } catch (error) {
      console.error(`Failed to ${isEditMode ? 'update' : 'create'} task:`, error)
    } finally {
      setIsSubmitting(false)
    }
  }

  const selectedAgent = agents.find((agent) => agent.id === formData.agentId)

  // 動的なテキスト
  const modalTitle = isEditMode ? t('backgroundAgent.editTask') : t('backgroundAgent.form.title')
  const submitButtonText = isEditMode ? t('common.update') : t('common.create')
  const submitLoadingText = isEditMode ? t('common.updating') : t('common.creating')

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 overflow-y-auto h-full w-full z-50">
      <div className="relative top-20 mx-auto p-5 w-full max-w-5xl">
        <div className="border-[0.5px] border-surface rounded-container shadow-xl bg-surface">
          <div className="flex items-center justify-between p-3 border-b border-subtle">
            <h3 className="text-heading font-medium text-ink">{modalTitle}</h3>
            <button
              onClick={onCancel}
              className="text-ink-faint hover:text-ink-muted transition-colors"
            >
              <XMarkIcon className="h-4 w-4" />
            </button>
          </div>

          <div className="p-3">
            <form onSubmit={handleSubmit} className="space-y-3">
              {/* Task Name - Full width */}
              <div className="col-span-2">
                <label className="block text-sm font-medium text-ink mb-1">
                  {t('backgroundAgent.form.taskName')}
                </label>
                <input
                  type="text"
                  value={formData.name}
                  onChange={(e) => setFormData((prev) => ({ ...prev, name: e.target.value }))}
                  className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-accent focus:border-accent bg-surface text-ink"
                  placeholder={t('backgroundAgent.form.taskNamePlaceholder')}
                />
                {errors.name && <p className="text-danger text-sm mt-1">{errors.name}</p>}
              </div>

              {/* 2-column grid for form fields */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                {/* Left Column - Basic Settings */}
                <div className="space-y-2">
                  {/* Cron Expression */}
                  <div>
                    <label className="block text-sm font-medium text-ink mb-1">
                      {t('backgroundAgent.form.schedule')}
                    </label>
                    <select
                      value={formData.cronExpression}
                      onChange={(e) =>
                        setFormData((prev) => ({ ...prev, cronExpression: e.target.value }))
                      }
                      className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-accent focus:border-accent bg-surface text-ink mb-2"
                    >
                      {CRON_PRESETS.map((preset) => (
                        <option key={preset.value} value={preset.value}>
                          {preset.label} ({preset.value})
                        </option>
                      ))}
                    </select>
                    <input
                      type="text"
                      value={formData.cronExpression}
                      onChange={(e) =>
                        setFormData((prev) => ({ ...prev, cronExpression: e.target.value }))
                      }
                      className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-accent focus:border-accent bg-surface text-ink"
                      placeholder="0 9 * * 1-5"
                    />
                    {errors.cronExpression && (
                      <p className="text-danger text-sm mt-1">{errors.cronExpression}</p>
                    )}
                    <p className="text-ink-muted text-xs mt-1">
                      {t('backgroundAgent.form.cronHelp')}
                    </p>
                  </div>

                  {/* Agent Selection */}
                  <div>
                    <label className="block text-sm font-medium text-ink mb-1">
                      {t('backgroundAgent.form.agent')}
                    </label>

                    <AgentSelector
                      agents={agents}
                      value={formData.agentId}
                      onChange={(agentId) => setFormData((prev) => ({ ...prev, agentId }))}
                      alignment="left"
                      showEditAgentsLink={false}
                      openDirection="down"
                    />

                    {errors.agentId && <p className="text-danger text-sm mt-1">{errors.agentId}</p>}
                    {selectedAgent && (
                      <p className="text-ink-muted text-sm mt-1">{t(selectedAgent.description)}</p>
                    )}
                  </div>
                </div>

                {/* Right Column - Execution Settings */}
                <div className="space-y-2">
                  {/* Project Directory */}
                  <div>
                    <label className="block text-sm font-medium text-ink mb-1">
                      {t('backgroundAgent.form.projectDirectory')}
                    </label>
                    <div className="w-full px-3 py-2 border border-strong rounded-control shadow-sm bg-surface">
                      <DirectorySelector
                        projectPath={
                          formData.projectDirectory ||
                          t('backgroundAgent.form.selectProjectDirectory')
                        }
                        onSelectDirectory={handleSelectDirectory}
                        onOpenIgnoreModal={handleOpenIgnoreModal}
                      />
                    </div>
                    <p className="text-ink-muted text-xs mt-1">
                      {t('backgroundAgent.form.projectDirectoryHelp')}
                    </p>
                  </div>

                  {/* Model Selection */}
                  <div>
                    <label className="block text-sm font-medium text-ink mb-1">
                      {t('backgroundAgent.form.model')}
                    </label>
                    <div className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus-within:outline-none focus-within:ring-accent focus-within:border-accent bg-surface">
                      <ModelSelector
                        openable={true}
                        value={formData.modelId}
                        onChange={(modelId) => setFormData((prev) => ({ ...prev, modelId }))}
                        className="w-full"
                      />
                    </div>
                    {errors.modelId && <p className="text-danger text-sm mt-1">{errors.modelId}</p>}
                  </div>

                  {/* Max Output Tokens */}
                  <div>
                    <label className="block text-sm font-medium text-ink mb-1">
                      {t('backgroundAgent.form.maxTokens')}
                    </label>
                    <input
                      type="number"
                      min="1"
                      max={modelMaxTokens}
                      value={formData.maxTokens}
                      onChange={(e) =>
                        setFormData((prev) => ({
                          ...prev,
                          maxTokens: parseInt(e.target.value) || 1
                        }))
                      }
                      className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-accent focus:border-accent bg-surface text-ink"
                      placeholder="4096"
                    />
                    {errors.maxTokens && (
                      <p className="text-danger text-sm mt-1">{errors.maxTokens}</p>
                    )}
                    <p className="text-ink-muted text-xs mt-1">
                      {t('backgroundAgent.form.maxTokensHelp')}
                    </p>
                  </div>
                </div>
              </div>

              {/* Wake Word - Full width */}
              <div>
                <label className="block text-sm font-medium text-ink mb-1">
                  {t('backgroundAgent.form.wakeWord')}
                </label>
                <textarea
                  value={formData.wakeWord}
                  onChange={(e) => setFormData((prev) => ({ ...prev, wakeWord: e.target.value }))}
                  rows={3}
                  className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-accent focus:border-accent bg-surface text-ink"
                  placeholder={t('backgroundAgent.form.wakeWordPlaceholder')}
                />
                {errors.wakeWord && <p className="text-danger text-sm mt-1">{errors.wakeWord}</p>}
                <p className="text-ink-muted text-xs mt-1">
                  {t('backgroundAgent.form.wakeWordHelp')}
                </p>
              </div>

              {/* Continue Session Toggle */}
              <div className="flex items-center">
                <input
                  type="checkbox"
                  id="continueSession"
                  checked={formData.continueSession}
                  onChange={(e) =>
                    setFormData((prev) => ({ ...prev, continueSession: e.target.checked }))
                  }
                  className="h-4 w-4 text-accent focus:ring-accent border-strong rounded-control"
                />
                <label htmlFor="continueSession" className="ml-2 block text-sm text-ink">
                  {t('backgroundAgent.form.continueSession')}
                </label>
              </div>
              <p className="text-ink-muted text-xs mt-1">
                {t('backgroundAgent.form.continueSessionHelp')}
              </p>

              {/* Continue Session Prompt - Only show when continueSession is true */}
              {formData.continueSession && (
                <div>
                  <label className="block text-sm font-medium text-ink mb-1">
                    {t('backgroundAgent.form.continueSessionPrompt')}
                  </label>
                  <textarea
                    value={formData.continueSessionPrompt}
                    onChange={(e) =>
                      setFormData((prev) => ({ ...prev, continueSessionPrompt: e.target.value }))
                    }
                    rows={3}
                    className="w-full px-3 py-2 border border-strong rounded-control shadow-sm focus:outline-none focus:ring-accent focus:border-accent bg-surface text-ink"
                    placeholder={t('backgroundAgent.form.continueSessionPromptPlaceholder')}
                  />
                  <p className="text-ink-muted text-xs mt-1">
                    {t('backgroundAgent.form.continueSessionPromptHelp')}
                  </p>
                </div>
              )}

              {/* Enabled Toggle */}
              <div className="flex items-center">
                <input
                  type="checkbox"
                  id="enabled"
                  checked={formData.enabled}
                  onChange={(e) => setFormData((prev) => ({ ...prev, enabled: e.target.checked }))}
                  className="h-4 w-4 text-accent focus:ring-accent border-strong rounded-control"
                />
                <label htmlFor="enabled" className="ml-2 block text-sm text-ink">
                  {t('backgroundAgent.form.enableTask')}
                </label>
              </div>

              {/* Form Actions */}
              <div className="flex justify-end space-x-3 pt-4">
                <button
                  type="button"
                  onClick={onCancel}
                  className="px-2.5 py-1 border border-strong rounded-control shadow-sm text-sm font-medium text-ink bg-surface hover:bg-surface-2 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent transition-colors"
                >
                  {t('common.cancel')}
                </button>
                <button
                  type="submit"
                  disabled={isSubmitting}
                  className="px-2.5 py-1 border border-transparent rounded-control shadow-sm text-sm font-medium text-accent-fg bg-accent hover:bg-accent-strong focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  {isSubmitting ? submitLoadingText : submitButtonText}
                </button>
              </div>
            </form>
          </div>
        </div>
      </div>

      {/* Ignore Settings Modal */}
      <IgnoreSettingsModal
        isOpen={showProjectIgnoreModal}
        onClose={() => setShowProjectIgnoreModal(false)}
        projectPath={formData.projectDirectory}
      />
    </div>
  )
}

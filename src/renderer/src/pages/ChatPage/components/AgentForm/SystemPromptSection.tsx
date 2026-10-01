import React, { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { FiZap, FiEye, FiEyeOff, FiChevronDown, FiChevronUp, FiMic } from 'react-icons/fi'
import { ToggleSwitch } from 'flowbite-react'
import { SystemPromptSectionProps } from './types'
import { replacePlaceholders } from '../../../../../../common/utils/placeholderUtils'
import { motion } from 'framer-motion'
import { SystemPromptBuilder } from '@/common/agents/toolRuleGenerator'

const PLACEHOLDERS = [
  { key: 'projectPath', translationKey: 'projectPathPlaceholder' },
  { key: 'date', translationKey: 'datePlaceholder' },
  { key: 'allowedCommands', translationKey: 'allowedCommandsPlaceholder' },
  { key: 'knowledgeBases', translationKey: 'knowledgeBasesPlaceholder' },
  { key: 'bedrockAgents', translationKey: 'bedrockAgentsPlaceholder' },
  { key: 'flows', translationKey: 'flowsPlaceholder' }
]

export const SystemPromptSection: React.FC<SystemPromptSectionProps> = ({
  system,
  name,
  description,
  additionalInstruction,
  environmentContextSettings,
  onChange,
  onAdditionalInstructionChange,
  onEnvironmentContextSettingsChange,
  onAutoGenerate,
  onVoiceChatGenerate,
  isGenerating,
  isGeneratingVoiceChat,
  projectPath,
  allowedCommands,
  knowledgeBases,
  bedrockAgents,
  flows = [],
  tools
}) => {
  const { t } = useTranslation()
  const [showPreview, setShowPreview] = useState(false)
  const [showAdditionalInstructionForm, setShowAdditionalInstructionForm] = useState(false)
  const [environmentContextText, setEnvironmentContextText] = useState<string>('')

  const getPreviewText = (text: string): string => {
    if (!text) return text
    const path = projectPath || t('noProjectPath')
    return replacePlaceholders(text, {
      projectPath: path,
      allowedCommands,
      knowledgeBases,
      bedrockAgents,
      flows
    })
  }

  // Load environment context asynchronously
  useEffect(() => {
    const loadEnvironmentContext = async () => {
      try {
        const path = projectPath || t('noProjectPath')
        const environmentContext = await SystemPromptBuilder.generateEnvironmentContext(
          environmentContextSettings
        )
        const replacedContext = replacePlaceholders(environmentContext, {
          projectPath: path,
          allowedCommands,
          knowledgeBases,
          bedrockAgents,
          flows
        })
        setEnvironmentContextText(replacedContext)
      } catch (error) {
        console.error('Failed to load environment context:', error)
        setEnvironmentContextText('Error loading environment context')
      }
    }

    loadEnvironmentContext()
  }, [
    tools,
    environmentContextSettings,
    projectPath,
    allowedCommands,
    knowledgeBases,
    bedrockAgents,
    flows,
    t
  ])

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
  }

  const togglePreview = () => {
    setShowPreview(!showPreview)
  }

  const toggleAdditionalInstructionForm = () => {
    setShowAdditionalInstructionForm(!showAdditionalInstructionForm)
  }

  const handleAdditionalInstructionChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    if (onAdditionalInstructionChange) {
      onAdditionalInstructionChange(e.target.value)
    }
  }

  return (
    <div>
      <div className="flex justify-between pb-2">
        <div className="flex-grow">
          <div className="flex items-center gap-2">
            <label className="block text-sm font-medium text-ink">{t('systemPrompt')}</label>
            <div className="flex justify-end items-center space-x-2">
              <div
                onClick={togglePreview}
                className="inline-flex items-center text-ink-muted hover:text-ink
                hover:text-ink cursor-pointer transition-colors duration-200"
                title={showPreview ? t('hidePreview') : t('showPreview')}
              >
                {showPreview ? <FiEyeOff className="w-4 h-4" /> : <FiEye className="w-4 h-4" />}
              </div>
              {name && description && (
                <>
                  <motion.button
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    onClick={onAutoGenerate}
                    disabled={isGenerating}
                    className="inline-flex items-center text-xs bg-accent-tint hover:bg-accent-tint-strong
                    text-accent rounded-control px-2 py-0.5 transition-colors duration-200 border border-accent"
                  >
                    <FiZap className={`w-3 h-3 mr-1 ${isGenerating ? 'animate-pulse' : ''}`} />
                    <span>{isGenerating ? t('generating') : t('generateSystemPrompt')}</span>
                  </motion.button>

                  <motion.button
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    onClick={onVoiceChatGenerate}
                    disabled={isGeneratingVoiceChat}
                    className="inline-flex items-center text-xs bg-success-soft hover:bg-success-soft-strong
                    text-success rounded-control px-2 py-0.5 transition-colors duration-200 border border-success"
                    title={t('generateVoiceChatPromptTooltip')}
                  >
                    <FiMic
                      className={`w-3 h-3 mr-1 ${isGeneratingVoiceChat ? 'animate-pulse' : ''}`}
                    />
                    <span>
                      {isGeneratingVoiceChat ? t('generating') : t('generateVoiceChatPrompt')}
                    </span>
                  </motion.button>
                </>
              )}

              {/* Additional Instruction Toggle Button */}
              {name && description && (
                <button
                  type="button"
                  onClick={toggleAdditionalInstructionForm}
                  className="text-xs flex items-center gap-1 text-ink-muted hover:text-ink"
                >
                  {showAdditionalInstructionForm ? (
                    <FiChevronUp className="w-3 h-3" />
                  ) : (
                    <FiChevronDown className="w-3 h-3" />
                  )}
                  <span>{t('additionalInstruction')}</span>
                </button>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Additional Instruction Form */}
      {showAdditionalInstructionForm && (
        <motion.div className="mb-4" initial={{ opacity: 0.5 }} animate={{ opacity: 1 }}>
          <p className="text-xs text-ink-muted mb-2">
            {t(
              'additionalInstructionInfo',
              'Optional instructions to guide the system prompt generation. These will be included when auto-generating the system prompt.'
            )}
          </p>
          <textarea
            value={additionalInstruction || ''}
            onChange={handleAdditionalInstructionChange}
            disabled={isGenerating || isGeneratingVoiceChat}
            className={`block w-full rounded-control border-strong bg-surface
              text-ink shadow-sm focus:border-accent focus:ring-accent sm:text-sm
              h-[150px] ${isGenerating || isGeneratingVoiceChat ? 'opacity-50 cursor-not-allowed' : ''}`}
            placeholder={t(
              'additionalInstructionPlaceholder',
              'Enter additional instructions for system prompt generation...'
            )}
          />
        </motion.div>
      )}

      <p className="text-xs text-ink-muted whitespace-pre-line mb-2 mt-1">
        {t('systemPromptInfo')}
      </p>
      <div className="p-2 bg-surface rounded-control border border-strong mb-2">
        <p className="text-xs text-ink-muted font-medium">{t('placeholders')}</p>
        <div className="grid lg:grid-cols-2 grid-cols-1 gap-1">
          {PLACEHOLDERS.map(({ key, translationKey }) => (
            <div key={key} className="flex items-center space-x-2">
              <code className="text-xs bg-raised px-2 py-1 rounded-control border border-strong text-ink">
                {`{{${key}}}`}
              </code>
              <span className="text-xs text-ink-muted">{t(translationKey)}</span>
              <button
                type="button"
                onClick={() => copyToClipboard(`{{${key}}}`)}
                className="text-xs text-ink-muted hover:text-ink"
              >
                {t('copy')}
              </button>
            </div>
          ))}
        </div>
      </div>

      <div className={showPreview ? 'grid grid-cols-2 gap-4 pt-2' : ''}>
        <div className={showPreview ? 'space-y-2' : ''}>
          <p className="text-xs text-ink-muted font-medium mb-2">{t('inputSystemPrompt')}</p>

          <textarea
            value={system}
            onChange={(e) => onChange(e.target.value)}
            disabled={isGenerating || isGeneratingVoiceChat}
            className={`block w-full rounded-control border-strong bg-surface
              text-ink shadow-sm focus:border-accent focus:ring-accent sm:text-sm
              h-[512px] ${isGenerating || isGeneratingVoiceChat ? 'opacity-50 cursor-not-allowed' : ''}`}
            required
            placeholder={t('systemPromptPlaceholder')}
          />
        </div>

        {/* Preview Column */}
        {showPreview && (
          <div>
            <p className="text-xs text-ink-muted font-medium mb-2">{t('previewResult')}</p>
            <div className="bg-surface-2 p-2.5 rounded-container border border-subtle h-[512px] overflow-y-auto text-sm text-ink whitespace-pre-wrap">
              {/* User Input Section */}
              <div className="mb-3">
                <p className="text-xs text-ink-muted font-medium mb-3 uppercase tracking-wide">
                  {t('userInput')}
                </p>
                <div className="whitespace-pre-wrap bg-surface p-3 rounded-control border border-subtle">
                  {getPreviewText(system)}
                </div>
              </div>

              {/* Environment Context Section */}
              <div>
                <p className="text-xs text-ink-muted font-medium mb-3 uppercase tracking-wide">
                  {t('autoAddedEnvironmentContext')}
                </p>
                <div className="whitespace-pre-wrap bg-raised p-3 rounded-control border border-subtle text-ink-muted">
                  {environmentContextText}
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Environment Context Settings */}
      <div className="mt-4 p-3 bg-surface-2 rounded-control border border-subtle">
        <h4 className="text-sm font-medium text-ink mb-3">{t('Environment Context Settings')}</h4>
        <p className="text-xs text-ink-muted mb-3">
          {t(
            'Choose which environment context sections to include in the system prompt. Basic context (project path, date) is always included.'
          )}
        </p>
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">{t('Project Rule')}</span>
              <p className="text-xs text-ink-muted mt-1">
                {t(
                  'Includes instructions to load project-specific rules from .bedrock-engineer/rules folder'
                )}
              </p>
            </div>
            <ToggleSwitch
              checked={Boolean(environmentContextSettings?.projectRule)}
              onChange={(checked) => {
                if (onEnvironmentContextSettingsChange) {
                  onEnvironmentContextSettingsChange({
                    projectRule: checked,
                    visualExpressionRules: environmentContextSettings?.visualExpressionRules ?? true
                  })
                }
              }}
              label=""
            />
          </div>

          <div className="flex items-center justify-between">
            <div>
              <span className="text-sm text-ink">{t('Visual Expression Rules')}</span>
              <p className="text-xs text-ink-muted mt-1">
                {t(
                  'Includes instructions for creating diagrams, images, and mathematical formulas'
                )}
              </p>
            </div>
            <ToggleSwitch
              checked={Boolean(environmentContextSettings?.visualExpressionRules)}
              onChange={(checked) => {
                if (onEnvironmentContextSettingsChange) {
                  onEnvironmentContextSettingsChange({
                    projectRule: environmentContextSettings?.projectRule ?? true,
                    visualExpressionRules: checked
                  })
                }
              }}
              label=""
            />
          </div>
        </div>
      </div>
    </div>
  )
}

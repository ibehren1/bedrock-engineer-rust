import React, { useEffect, useState } from 'react'
import toast from 'react-hot-toast'
import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import { SettingSelect } from '../SettingSelect'
import {
  BEDROCK_EMBEDDING_MODELS,
  EmbeddingProvider,
  KnowledgeSettings,
  LOCAL_EMBEDDING_MODEL,
  LocalModelStatus
} from '@/types/knowledge'
import { formatBytes } from '@renderer/components/KnowledgeBase/knowledgeSettings'

const buttonClass =
  'text-xs px-2.5 py-1.5 rounded-control border border-subtle text-ink hover:bg-raised disabled:opacity-50'
const accentButtonClass =
  'text-xs px-2.5 py-1.5 rounded-control border border-accent bg-accent-tint hover:bg-accent-tint-strong text-accent disabled:opacity-50'

const LocalModelControls: React.FC = () => {
  const { t } = useTranslation()
  const [status, setStatus] = useState<LocalModelStatus | null>(null)
  const [progress, setProgress] = useState<{ received: number; total: number } | null>(null)

  const refresh = () =>
    window.api.knowledge
      .localModelStatus()
      .then(setStatus)
      .catch(() => {})

  useEffect(() => {
    refresh()
    return window.api.knowledge.onModelDownload((p) => {
      if (p.state === 'downloading') {
        setProgress({ received: p.received, total: p.total })
      } else {
        setProgress(null)
        refresh()
        if (p.state === 'done') toast.success(t('knowledgeBase.settings.downloadDone'))
        else if (p.error) toast.error(p.error)
      }
    })
  }, [t])

  if (!status) return null
  const downloading = status.downloading || progress !== null
  const percent =
    progress && progress.total > 0 ? Math.floor((progress.received / progress.total) * 100) : 0

  return (
    <div className="space-y-1.5">
      <label className="block text-sm font-medium text-ink">
        {t('knowledgeBase.settings.localModel')}: {LOCAL_EMBEDDING_MODEL.id}
      </label>
      <p className="text-xs text-ink-muted">
        {t('knowledgeBase.settings.localModelHint', { size: formatBytes(status.totalBytes) })}
      </p>
      <div className="flex items-center gap-2">
        <span
          className={`text-xs ${status.installed ? 'text-success' : status.supported ? 'text-ink-muted' : 'text-warning'}`}
        >
          {!status.supported
            ? t('knowledgeBase.settings.localUnsupported')
            : status.installed
              ? t('knowledgeBase.settings.localInstalled', { size: formatBytes(status.bytes) })
              : downloading
                ? t('knowledgeBase.settings.downloading', { percent })
                : t('knowledgeBase.settings.localNotInstalled')}
        </span>
        {status.supported && !status.installed && !downloading && (
          <button
            type="button"
            className={accentButtonClass}
            onClick={() => {
              setProgress({ received: 0, total: status.totalBytes })
              window.api.knowledge.downloadLocalModel().catch(() => {
                // The error arrives as a `knowledge:model-download` event too.
                setProgress(null)
                refresh()
              })
            }}
          >
            {t('knowledgeBase.settings.download')}
          </button>
        )}
        {downloading && (
          <button
            type="button"
            className={buttonClass}
            onClick={() => window.api.knowledge.deleteLocalModel().finally(refresh)}
          >
            {t('knowledgeBase.settings.cancelDownload')}
          </button>
        )}
        {status.installed && (
          <button
            type="button"
            className={buttonClass}
            onClick={() => {
              if (window.confirm(t('knowledgeBase.settings.deleteModelConfirm')))
                window.api.knowledge
                  .deleteLocalModel()
                  .catch((e) => toast.error(String(e)))
                  .finally(refresh)
            }}
          >
            {t('knowledgeBase.settings.deleteModel')}
          </button>
        )}
      </div>
      {downloading && (
        <div className="h-1.5 rounded-full bg-surface-2 overflow-hidden">
          <div className="h-full bg-accent transition-all" style={{ width: `${percent}%` }} />
        </div>
      )}
    </div>
  )
}

export const KnowledgeEmbeddingSection: React.FC<{
  settings: KnowledgeSettings
  onChange: (next: KnowledgeSettings) => void
}> = ({ settings, onChange }) => {
  const { t } = useTranslation()
  const e = settings.embedding
  const model =
    BEDROCK_EMBEDDING_MODELS.find((m) => m.id === e.bedrockModelId) ?? BEDROCK_EMBEDDING_MODELS[0]
  const setEmbedding = (patch: Partial<KnowledgeSettings['embedding']>) =>
    onChange({ ...settings, embedding: { ...e, ...patch } })

  return (
    <SettingSection
      title={t('knowledgeBase.settings.embeddingTitle')}
      description={t('knowledgeBase.settings.embeddingDescription')}
    >
      <SettingSelect
        label={t('knowledgeBase.settings.provider')}
        value={e.provider}
        options={[
          { value: 'bedrock', label: t('knowledgeBase.settings.providerBedrock') },
          { value: 'local', label: t('knowledgeBase.settings.providerLocal') }
        ]}
        onChange={(ev) => setEmbedding({ provider: ev.target.value as EmbeddingProvider })}
      />
      {e.provider === 'bedrock' ? (
        <>
          <SettingSelect
            label={t('knowledgeBase.settings.bedrockModel')}
            description={t('knowledgeBase.settings.bedrockModelHint')}
            value={model.id}
            options={BEDROCK_EMBEDDING_MODELS.map((m) => ({ value: m.id, label: m.label }))}
            onChange={(ev) => {
              const next = BEDROCK_EMBEDDING_MODELS.find((m) => m.id === ev.target.value)
              if (!next) return
              setEmbedding({
                bedrockModelId: next.id,
                dimensions: next.dimensions.includes(e.dimensions)
                  ? e.dimensions
                  : next.dimensions[0]
              })
            }}
          />
          <SettingSelect
            label={t('knowledgeBase.settings.dimensions')}
            value={String(
              model.dimensions.includes(e.dimensions) ? e.dimensions : model.dimensions[0]
            )}
            options={model.dimensions.map((d) => ({ value: String(d), label: String(d) }))}
            disabled={model.dimensions.length === 1}
            onChange={(ev) => setEmbedding({ dimensions: Number(ev.target.value) })}
          />
        </>
      ) : null}
      <LocalModelControls />
    </SettingSection>
  )
}

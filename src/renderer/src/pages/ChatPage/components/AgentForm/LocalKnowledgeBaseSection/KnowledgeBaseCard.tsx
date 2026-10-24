import React, { useCallback, useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  FiChevronDown,
  FiChevronRight,
  FiEdit2,
  FiFile,
  FiGlobe,
  FiRefreshCw,
  FiSearch,
  FiSliders,
  FiTool,
  FiTrash2,
  FiUpload,
  FiX
} from 'react-icons/fi'
import type { KbInfo, KnowledgeJob, KnowledgeSearchHit, SourceInfo } from '@/types/knowledge'
import {
  embeddingLabel,
  formatBytes,
  sameSpec
} from '@renderer/components/KnowledgeBase/knowledgeSettings'
import type { EmbeddingSpec, KbStats, KnowledgeSettings, UrlOptions } from '@/types/knowledge'
import {
  compactNumber,
  formatDuration,
  HealthChip,
  HealthList,
  KbStatTiles,
  relativeTime
} from '@renderer/components/KnowledgeBase/KbStatsWidgets'
import { NumberInput } from '@renderer/components/KnowledgeBase/NumberInput'
import { jobKey, useLocalKnowledgeBases } from './useLocalKnowledgeBases'

type Kb = ReturnType<typeof useLocalKnowledgeBases>

const buttonClass =
  'inline-flex items-center gap-1 text-xs px-2 py-1 rounded-control border border-subtle text-ink hover:bg-raised disabled:opacity-50'
const accentButtonClass =
  'inline-flex items-center gap-1 text-xs px-2 py-1 rounded-control border border-accent bg-accent-tint hover:bg-accent-tint-strong text-accent disabled:opacity-50'
const inputClass =
  'bg-surface border border-strong text-ink text-sm rounded-control focus:ring-accent focus:border-accent block w-full px-2 py-1.5'

/** Enter in an input inside the agent form must not submit the form. */
const noSubmit = (onEnter?: () => void) => (e: React.KeyboardEvent) => {
  if (e.key === 'Enter') {
    e.preventDefault()
    onEnter?.()
  }
}

const statusClass: Record<SourceInfo['status'], string> = {
  queued: 'text-ink-muted',
  processing: 'text-accent',
  ready: 'text-success',
  error: 'text-danger',
  cancelled: 'text-warning'
}

const JobProgress: React.FC<{ job: KnowledgeJob; onCancel: () => void }> = ({ job, onCancel }) => {
  const { t } = useTranslation()
  const percent = job.total > 0 ? Math.min(100, Math.round((job.current / job.total) * 100)) : 0
  return (
    <div className="flex items-center gap-2 mt-1">
      <div className="flex-1 h-1.5 rounded-full bg-surface-2 overflow-hidden">
        <div
          className={`h-full bg-accent transition-all ${job.total === 0 ? 'animate-pulse w-1/3' : ''}`}
          style={job.total > 0 ? { width: `${percent}%` } : undefined}
        />
      </div>
      <span className="text-xs text-ink-muted whitespace-nowrap">
        {t(`knowledgeBase.phase.${job.phase}`, { current: job.current, total: job.total })}
      </span>
      <button type="button" className={buttonClass} onClick={onCancel}>
        <FiX className="w-3 h-3" />
        {t('knowledgeBase.cancelJob')}
      </button>
    </div>
  )
}

/** Link depth, page limit and "stay under this path" inputs for a website source. */
const CrawlOptionsFields: React.FC<{
  value: UrlOptions
  onChange: (value: UrlOptions) => void
}> = ({ value, onChange }) => {
  const { t } = useTranslation()
  return (
    <div className="flex flex-wrap items-center gap-3 text-xs text-ink-muted">
      <label className="flex items-center gap-1">
        {t('knowledgeBase.depth')}
        <NumberInput
          min={0}
          max={10}
          className={`${inputClass} !w-16`}
          value={value.maxDepth}
          onChange={(maxDepth) => onChange({ ...value, maxDepth })}
        />
      </label>
      <label className="flex items-center gap-1">
        {t('knowledgeBase.maxPages')}
        <NumberInput
          min={1}
          max={1000}
          className={`${inputClass} !w-20`}
          value={value.maxPages}
          onChange={(maxPages) => onChange({ ...value, maxPages })}
        />
      </label>
      <label className="flex items-center gap-1" title={t('knowledgeBase.pathPrefixOnlyHint')}>
        <input
          type="checkbox"
          checked={value.pathPrefixOnly}
          onChange={(e) => onChange({ ...value, pathPrefixOnly: e.target.checked })}
        />
        {t('knowledgeBase.pathPrefixOnly')}
      </label>
    </div>
  )
}

const MaintenancePanel: React.FC<{
  stats: KbStats
  busy: boolean
  onOptimize: () => Promise<void>
}> = ({ stats, busy, onOptimize }) => {
  const { t, i18n } = useTranslation()
  const [running, setRunning] = useState(false)
  const { usage, health } = stats
  const issues = health.issues.filter((i) => i.kind !== 'empty')
  const job = usage.lastJob
  return (
    <div className="rounded-container border border-subtle bg-surface-2 p-2.5 space-y-2">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 space-y-0.5">
          <h5 className="text-xs font-medium text-ink">{t('knowledgeBase.maintenance.title')}</h5>
          <p className="text-xs text-ink-muted">
            {usage.lastOptimizedAt
              ? t('knowledgeBase.maintenance.lastOptimized', {
                  time: relativeTime(usage.lastOptimizedAt, i18n.language)
                })
              : t('knowledgeBase.maintenance.neverOptimized')}
            {job &&
              ` · ${t('knowledgeBase.maintenance.lastJob', {
                time: relativeTime(job.at, i18n.language),
                chunks: compactNumber(job.chunks),
                duration: formatDuration(job.durationMs)
              })}`}
          </p>
        </div>
        <button
          type="button"
          className={buttonClass}
          disabled={running || busy}
          title={t('knowledgeBase.maintenance.optimizeHint')}
          onClick={async () => {
            setRunning(true)
            try {
              await onOptimize()
            } finally {
              setRunning(false)
            }
          }}
        >
          <FiTool className="w-3 h-3" />
          {running
            ? t('knowledgeBase.maintenance.optimizing')
            : t('knowledgeBase.maintenance.optimize')}
        </button>
      </div>
      <HealthList issues={issues} />
    </div>
  )
}

const SourceRow: React.FC<{
  source: SourceInfo
  job?: KnowledgeJob
  onResync: () => void
  onRemove: () => void
  onCancel: (jobId: string) => void
  onSaveOptions: (options: UrlOptions, resync: boolean) => Promise<void>
}> = ({ source, job, onResync, onRemove, onCancel, onSaveOptions }) => {
  const { t } = useTranslation()
  const Icon = source.kind === 'url' ? FiGlobe : FiFile
  const status = job ? 'processing' : source.status
  const [editing, setEditing] = useState<UrlOptions | null>(null)
  const save = async (resync: boolean) => {
    if (!editing) return
    await onSaveOptions(editing, resync)
    setEditing(null)
  }
  return (
    <li className="py-2">
      <div className="flex items-start gap-2">
        <Icon className="w-4 h-4 mt-0.5 text-ink-muted flex-shrink-0" />
        <div className="min-w-0 flex-1">
          <div className="text-sm text-ink truncate" title={source.title}>
            {source.title}
          </div>
          <div className="text-xs text-ink-muted truncate" title={source.uri}>
            {source.uri}
          </div>
          <div className="text-xs text-ink-muted flex flex-wrap gap-x-2">
            <span className={statusClass[status]}>{t(`knowledgeBase.status.${status}`)}</span>
            {source.documentCount > 0 && (
              <span>{t('knowledgeBase.documents', { count: source.documentCount })}</span>
            )}
            {source.chunkCount > 0 && (
              <span>{t('knowledgeBase.passages', { count: source.chunkCount })}</span>
            )}
            {source.syncedAt && (
              <span>
                {t('knowledgeBase.syncedAt', { time: new Date(source.syncedAt).toLocaleString() })}
              </span>
            )}
          </div>
          {source.kind === 'url' && source.options && !editing && (
            <div className="text-xs text-ink-muted">
              {t('knowledgeBase.crawlOptions', {
                depth: source.options.maxDepth,
                pages: source.options.maxPages
              })}
              {source.options.pathPrefixOnly ? ` · ${t('knowledgeBase.pathPrefixOnly')}` : ''}
            </div>
          )}
          {editing && (
            <div className="mt-1 space-y-1.5">
              <CrawlOptionsFields value={editing} onChange={setEditing} />
              <div className="flex gap-2">
                <button type="button" className={accentButtonClass} onClick={() => save(true)}>
                  {t('knowledgeBase.saveAndResync')}
                </button>
                <button type="button" className={buttonClass} onClick={() => save(false)}>
                  {t('knowledgeBase.save')}
                </button>
                <button type="button" className={buttonClass} onClick={() => setEditing(null)}>
                  {t('knowledgeBase.cancel')}
                </button>
              </div>
            </div>
          )}
          {!job && source.error && source.status !== 'ready' && (
            <div className="text-xs text-danger mt-0.5 break-words">{source.error}</div>
          )}
          {job && <JobProgress job={job} onCancel={() => onCancel(job.jobId)} />}
        </div>
        {!job && !editing && (
          <div className="flex gap-1 flex-shrink-0">
            {source.kind === 'url' && (
              <button
                type="button"
                className={buttonClass}
                onClick={() =>
                  setEditing(source.options ?? { maxDepth: 2, maxPages: 50, pathPrefixOnly: true })
                }
                title={t('knowledgeBase.editCrawlOptions')}
              >
                <FiSliders className="w-3 h-3" />
              </button>
            )}
            <button
              type="button"
              className={buttonClass}
              onClick={onResync}
              title={t('knowledgeBase.resync')}
            >
              <FiRefreshCw className="w-3 h-3" />
            </button>
            <button
              type="button"
              className={buttonClass}
              onClick={() => {
                if (window.confirm(t('knowledgeBase.removeConfirm', { name: source.title })))
                  onRemove()
              }}
              title={t('knowledgeBase.remove')}
            >
              <FiTrash2 className="w-3 h-3" />
            </button>
          </div>
        )}
      </div>
    </li>
  )
}

const AddWebsite: React.FC<{
  defaults: KnowledgeSettings['crawler']
  onAdd: (url: string, options: UrlOptions) => Promise<boolean>
}> = ({ defaults, onAdd }) => {
  const { t } = useTranslation()
  const [url, setUrl] = useState('')
  const [options, setOptions] = useState<UrlOptions>({
    maxDepth: defaults.maxDepth,
    maxPages: defaults.maxPages,
    pathPrefixOnly: true
  })
  const [busy, setBusy] = useState(false)
  const submit = async () => {
    if (!url.trim() || busy) return
    setBusy(true)
    if (await onAdd(url.trim(), options)) setUrl('')
    setBusy(false)
  }
  return (
    <div className="space-y-1.5">
      <div className="flex gap-2">
        <input
          type="url"
          className={inputClass}
          placeholder={t('knowledgeBase.urlPlaceholder')}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={noSubmit(submit)}
        />
        <button
          type="button"
          className={accentButtonClass}
          disabled={!url.trim() || busy}
          onClick={submit}
        >
          <FiGlobe className="w-3 h-3" />
          <span className="whitespace-nowrap">{t('knowledgeBase.addWebsite')}</span>
        </button>
      </div>
      <CrawlOptionsFields value={options} onChange={setOptions} />
    </div>
  )
}

const SearchTester: React.FC<{ onSearch: (q: string) => Promise<KnowledgeSearchHit[]> }> = ({
  onSearch
}) => {
  const { t } = useTranslation()
  const [query, setQuery] = useState('')
  const [hits, setHits] = useState<KnowledgeSearchHit[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const submit = async () => {
    if (!query.trim() || busy) return
    setBusy(true)
    setError(null)
    try {
      setHits(await onSearch(query.trim()))
    } catch (e) {
      setHits(null)
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="space-y-2">
      <h5 className="text-xs font-medium text-ink">{t('knowledgeBase.searchTitle')}</h5>
      <div className="flex gap-2">
        <input
          className={inputClass}
          placeholder={t('knowledgeBase.searchPlaceholder')}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={noSubmit(submit)}
        />
        <button
          type="button"
          className={buttonClass}
          disabled={!query.trim() || busy}
          onClick={submit}
        >
          <FiSearch className="w-3 h-3" />
          <span className="whitespace-nowrap">
            {busy ? t('knowledgeBase.searching') : t('knowledgeBase.search')}
          </span>
        </button>
      </div>
      {error && <p className="text-xs text-danger">{error}</p>}
      {hits && hits.length === 0 && (
        <p className="text-xs text-ink-muted">{t('knowledgeBase.noResults')}</p>
      )}
      {hits && hits.length > 0 && (
        <ol className="space-y-2">
          {hits.map((h, i) => (
            <li key={`${h.sourceId}-${i}`} className="border border-subtle rounded-control p-2">
              <div className="flex justify-between gap-2 text-xs">
                <span className="font-medium text-ink truncate">
                  {h.title}
                  {h.headingPath ? ` › ${h.headingPath}` : ''}
                </span>
                <span className="text-ink-muted whitespace-nowrap">
                  {t('knowledgeBase.score', { score: h.score.toFixed(4) })}
                </span>
              </div>
              <div className="text-xs text-ink-muted truncate">{h.uri}</div>
              <p className="text-xs text-ink mt-1 whitespace-pre-wrap line-clamp-6">{h.text}</p>
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}

export const KnowledgeBaseCard: React.FC<{
  kb: KbInfo
  sources: SourceInfo[]
  jobs: Record<string, KnowledgeJob>
  actions: Kb
  currentSpec: EmbeddingSpec
  crawlerDefaults: KnowledgeSettings['crawler']
}> = ({ kb, sources, jobs, actions, currentSpec, crawlerDefaults }) => {
  const { t } = useTranslation()
  const [open, setOpen] = useState(sources.length === 0)
  const [editing, setEditing] = useState(false)
  const [name, setName] = useState(kb.name)
  const [description, setDescription] = useState(kb.description)
  const rebuildJob = jobs[jobKey({ kbId: kb.id })]
  const mismatch = !sameSpec(kb.embedding, currentSpec)
  const [stats, setStats] = useState<KbStats | null>(null)
  const reloadStats = useCallback(() => {
    window.api.knowledge
      .stats(kb.agentId, kb.id)
      .then(setStats)
      .catch(() => setStats(null))
  }, [kb.agentId, kb.id])
  // Counts and size change whenever the knowledgebase does.
  useEffect(reloadStats, [reloadStats, kb.bytes, kb.chunkCount, kb.sourceCount, sources])

  return (
    <div className="border border-subtle rounded-container bg-surface">
      <div className="flex items-start gap-2 p-2.5">
        <button
          type="button"
          className="mt-0.5 text-ink-muted"
          onClick={() => setOpen(!open)}
          aria-expanded={open}
        >
          {open ? <FiChevronDown className="w-4 h-4" /> : <FiChevronRight className="w-4 h-4" />}
        </button>
        <div className="flex-1 min-w-0">
          {editing ? (
            <div className="space-y-1.5">
              <input
                className={inputClass}
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={noSubmit()}
              />
              <input
                className={inputClass}
                value={description}
                placeholder={t('knowledgeBase.descriptionPlaceholder')}
                onChange={(e) => setDescription(e.target.value)}
                onKeyDown={noSubmit()}
              />
              <div className="flex gap-2">
                <button
                  type="button"
                  className={accentButtonClass}
                  disabled={!name.trim()}
                  onClick={async () => {
                    await actions.update(kb.id, name, description)
                    setEditing(false)
                  }}
                >
                  {t('knowledgeBase.save')}
                </button>
                <button
                  type="button"
                  className={buttonClass}
                  onClick={() => {
                    setName(kb.name)
                    setDescription(kb.description)
                    setEditing(false)
                  }}
                >
                  {t('knowledgeBase.cancel')}
                </button>
              </div>
            </div>
          ) : (
            <button type="button" className="text-left w-full" onClick={() => setOpen(!open)}>
              <div className="flex items-center gap-2 flex-wrap">
                <span className="text-sm font-medium text-ink">{kb.name}</span>
                {stats && <HealthChip issues={stats.health.issues} />}
              </div>
              {kb.description && <div className="text-xs text-ink-muted">{kb.description}</div>}
              <div className="text-xs text-ink-muted mt-0.5">
                {!open &&
                  `${t('knowledgeBase.stats.collapsed', {
                    chunks: compactNumber(kb.chunkCount),
                    size: formatBytes(kb.bytes)
                  })} · `}
                {embeddingLabel(kb.embedding)}
              </div>
            </button>
          )}
          {mismatch && !rebuildJob && (
            <div className="text-xs text-warning mt-1">
              {t('knowledgeBase.modelMismatch', {
                model: embeddingLabel(kb.embedding),
                current: embeddingLabel(currentSpec)
              })}
            </div>
          )}
          {rebuildJob && (
            <JobProgress job={rebuildJob} onCancel={() => actions.cancelJob(rebuildJob.jobId)} />
          )}
        </div>
        {!editing && (
          <div className="flex gap-1 flex-shrink-0">
            {mismatch && kb.chunkCount > 0 && !rebuildJob && (
              <button
                type="button"
                className={buttonClass}
                title={t('knowledgeBase.rebuildHint', { model: embeddingLabel(currentSpec) })}
                onClick={() => {
                  if (
                    window.confirm(
                      t('knowledgeBase.rebuildConfirm', {
                        name: kb.name,
                        model: embeddingLabel(currentSpec)
                      })
                    )
                  )
                    actions.rebuild(kb.id)
                }}
              >
                <FiRefreshCw className="w-3 h-3" />
                {t('knowledgeBase.rebuild')}
              </button>
            )}
            <button
              type="button"
              className={buttonClass}
              title={t('knowledgeBase.rename')}
              onClick={() => setEditing(true)}
            >
              <FiEdit2 className="w-3 h-3" />
            </button>
            <button
              type="button"
              className={buttonClass}
              title={t('knowledgeBase.delete')}
              onClick={() => {
                if (window.confirm(t('knowledgeBase.deleteConfirm', { name: kb.name })))
                  actions.remove(kb.id)
              }}
            >
              <FiTrash2 className="w-3 h-3 text-danger" />
            </button>
          </div>
        )}
      </div>

      {open && (
        <div className="border-t border-subtle p-2.5 space-y-3">
          {stats && <KbStatTiles stats={stats} />}
          {stats && (
            <MaintenancePanel
              stats={stats}
              busy={!!rebuildJob || Object.values(jobs).some((j) => j.kbId === kb.id)}
              onOptimize={async () => {
                await actions.optimize(kb.id)
                reloadStats()
              }}
            />
          )}
          <div className="space-y-2">
            <button
              type="button"
              className={accentButtonClass}
              onClick={() => actions.pickFiles(kb.id)}
            >
              <FiUpload className="w-3 h-3" />
              {t('knowledgeBase.addFiles')}
            </button>
            <AddWebsite
              defaults={crawlerDefaults}
              onAdd={async (url, options) =>
                (await actions.addUrl(kb.id, url, options)) !== undefined
              }
            />
          </div>

          <div>
            <h5 className="text-xs font-medium text-ink">{t('knowledgeBase.sources')}</h5>
            {sources.length === 0 ? (
              <p className="text-xs text-ink-muted mt-1">{t('knowledgeBase.noSources')}</p>
            ) : (
              <ul className="divide-y divide-subtle">
                {sources.map((s) => (
                  <SourceRow
                    key={s.id}
                    source={s}
                    job={jobs[s.id]}
                    onResync={() => actions.resync(kb.id, s.id)}
                    onRemove={() => actions.removeSource(kb.id, s.id)}
                    onCancel={(jobId) => actions.cancelJob(jobId)}
                    onSaveOptions={async (options, resync) => {
                      const saved = await actions.updateSourceOptions(kb.id, s.id, options)
                      if (saved && resync) await actions.resync(kb.id, s.id)
                    }}
                  />
                ))}
              </ul>
            )}
          </div>

          {kb.chunkCount > 0 && <SearchTester onSearch={(q) => actions.search(q, kb.id)} />}
        </div>
      )}
    </div>
  )
}

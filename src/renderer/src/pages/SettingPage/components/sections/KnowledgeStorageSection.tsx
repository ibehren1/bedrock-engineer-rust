import React, { useCallback, useEffect, useMemo, useState } from 'react'
import toast from 'react-hot-toast'
import { useTranslation } from 'react-i18next'
import { FiRefreshCw, FiTool, FiTrash2 } from 'react-icons/fi'
import { SettingSection } from '../SettingSection'
import useSetting from '@renderer/hooks/useSetting'
import type { KnowledgeStorageEntry, KnowledgeStorageOverview } from '@/types/knowledge'
import { embeddingLabel, formatBytes } from '@renderer/components/KnowledgeBase/knowledgeSettings'
import {
  compactNumber,
  formatDuration,
  HealthChip,
  Meter,
  relativeTime,
  Sparkline,
  StatTile
} from '@renderer/components/KnowledgeBase/KbStatsWidgets'

const buttonClass =
  'inline-flex items-center gap-1 text-xs px-2 py-1 rounded-control border border-subtle text-ink hover:bg-raised disabled:opacity-50'

const needsAttention = (e: KnowledgeStorageEntry) =>
  e.orphan ||
  (e.stats?.health.issues ?? []).some((i) => i.severity === 'critical' || i.severity === 'warning')

/** Element-wise sum of equal-length day series. */
const sumSeries = (series: number[][]): number[] => {
  const len = Math.max(0, ...series.map((s) => s.length))
  return Array.from({ length: len }, (_, i) => series.reduce((acc, s) => acc + (s[i] ?? 0), 0))
}

const median = (values: number[]): number | null => {
  if (values.length === 0) return null
  const v = [...values].sort((a, b) => a - b)
  return v[Math.floor((v.length - 1) / 2)]
}

/**
 * Settings > Knowledge Base overview: totals across every knowledgebase on disk, then one row
 * per knowledgebase with its usage, search time and health, and the maintenance actions.
 */
export const KnowledgeStorageSection: React.FC = () => {
  const { t, i18n } = useTranslation()
  const { customAgents, saveCustomAgents } = useSetting()
  const [overview, setOverview] = useState<KnowledgeStorageOverview | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  const refresh = useCallback(() => {
    window.api.knowledge
      .storageOverview()
      .then(setOverview)
      .catch((e) => console.error('Failed to load knowledge base overview:', e))
  }, [])

  useEffect(() => {
    refresh()
    return window.api.knowledge.onChanged(refresh)
  }, [refresh])

  const agentName = (id: string) => customAgents.find((a) => a.id === id)?.name ?? id
  const entries = overview?.knowledgeBases ?? []
  const orphans = entries.filter((e) => e.orphan)

  const totals = useMemo(() => {
    const stats = entries.flatMap((e) => (e.stats ? [e.stats] : []))
    return {
      kbs: entries.length,
      agents: new Set(entries.map((e) => e.agentId)).size,
      passages: entries.reduce((a, e) => a + e.chunkCount, 0),
      documents: stats.reduce((a, s) => a + s.info.documentCount, 0),
      reclaimable: stats.reduce((a, s) => a + s.health.reclaimableBytes, 0),
      daily: sumSeries(stats.map((s) => s.usage.dailySearches)),
      searches: stats.reduce((a, s) => a + s.usage.searchesInPeriod, 0),
      p50: median(
        stats.flatMap((s) => (s.usage.latencyP50Ms == null ? [] : [s.usage.latencyP50Ms]))
      ),
      attention: entries.filter(needsAttention).length,
      days: stats[0]?.periodDays ?? 30
    }
  }, [entries])

  const optimize = async (e: KnowledgeStorageEntry) => {
    setBusy(`${e.agentId}/${e.kbId}`)
    try {
      const r = await window.api.knowledge.optimize(e.agentId, e.kbId)
      if (r.integrity !== 'ok')
        toast.error(t('knowledgeBase.maintenance.integrityFailed', { result: r.integrity }))
      return r
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
      return undefined
    } finally {
      setBusy(null)
    }
  }

  const optimizeAll = async () => {
    let before = 0
    let after = 0
    for (const e of entries) {
      const r = await optimize(e)
      if (r) {
        before += r.bytesBefore
        after += r.bytesAfter
      }
    }
    toast.success(
      t('knowledgeBase.maintenance.optimized', {
        before: formatBytes(before),
        after: formatBytes(after)
      })
    )
    refresh()
  }

  const deleteKb = async (e: KnowledgeStorageEntry) => {
    if (!window.confirm(t('knowledgeBase.deleteConfirm', { name: e.name }))) return
    try {
      await window.api.knowledge.delete(e.agentId, e.kbId)
      // Keep the agent's list in step with what is on disk.
      const agent = customAgents.find((a) => a.id === e.agentId)
      if (agent?.localKnowledgeBases?.some((k) => k.id === e.kbId)) {
        saveCustomAgents(
          customAgents.map((a) =>
            a.id === e.agentId
              ? { ...a, localKnowledgeBases: a.localKnowledgeBases?.filter((k) => k.id !== e.kbId) }
              : a
          )
        )
      }
      toast.success(t('knowledgeBase.deleted'))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
    }
    refresh()
  }

  return (
    <SettingSection
      title={t('knowledgeBase.settings.storageTitle')}
      description={
        overview
          ? t('knowledgeBase.settings.storageDescription', {
              size: formatBytes(overview.totalBytes),
              path: overview.root
            })
          : undefined
      }
    >
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-2">
        <StatTile
          label={t('knowledgeBase.settings.overview.knowledgeBases')}
          value={compactNumber(totals.kbs)}
          sub={t('knowledgeBase.settings.overview.acrossAgents', { count: totals.agents })}
        />
        <StatTile
          label={t('knowledgeBase.stats.passages')}
          value={compactNumber(totals.passages)}
          sub={t('knowledgeBase.settings.overview.documents', {
            n: compactNumber(totals.documents)
          })}
        />
        <StatTile
          label={t('knowledgeBase.stats.storage')}
          value={formatBytes(overview?.totalBytes ?? 0)}
          sub={
            totals.reclaimable > 0
              ? t('knowledgeBase.stats.reclaimable', { size: formatBytes(totals.reclaimable) })
              : t('knowledgeBase.stats.compact')
          }
        >
          <Meter
            value={totals.reclaimable}
            max={overview?.totalBytes ?? 0}
            tone={
              overview && totals.reclaimable >= 0.2 * overview.totalBytes ? 'warning' : 'accent'
            }
            label={t('knowledgeBase.stats.reclaimable', { size: formatBytes(totals.reclaimable) })}
          />
        </StatTile>
        <StatTile
          label={t('knowledgeBase.stats.searches', { days: totals.days })}
          value={compactNumber(totals.searches)}
          sub={
            totals.p50 != null
              ? t('knowledgeBase.settings.overview.typicalTime', {
                  time: formatDuration(totals.p50)
                })
              : t('knowledgeBase.stats.noSearches')
          }
        >
          {totals.daily.length > 0 && (
            <Sparkline
              values={totals.daily}
              label={t('knowledgeBase.stats.searchesPerDay', { days: totals.days })}
            />
          )}
        </StatTile>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <button type="button" className={buttonClass} onClick={refresh}>
          <FiRefreshCw className="w-3 h-3" />
          {t('knowledgeBase.settings.refresh')}
        </button>
        {entries.length > 0 && (
          <button
            type="button"
            className={buttonClass}
            disabled={busy !== null}
            title={t('knowledgeBase.maintenance.optimizeHint')}
            onClick={optimizeAll}
          >
            <FiTool className="w-3 h-3" />
            {busy
              ? t('knowledgeBase.maintenance.optimizing')
              : t('knowledgeBase.settings.overview.optimizeAll')}
          </button>
        )}
        {orphans.length > 0 && (
          <button
            type="button"
            className={buttonClass}
            onClick={async () => {
              if (
                !window.confirm(t('knowledgeBase.settings.purgeConfirm', { count: orphans.length }))
              )
                return
              try {
                const n = await window.api.knowledge.purgeOrphans()
                toast.success(t('knowledgeBase.settings.purged', { count: n }))
              } catch (e) {
                toast.error(e instanceof Error ? e.message : String(e))
              }
              refresh()
            }}
          >
            <FiTrash2 className="w-3 h-3 text-danger" />
            {t('knowledgeBase.settings.purgeOrphans')}
          </button>
        )}
        {totals.attention > 0 && (
          <span className="text-xs text-ink-muted">
            {t('knowledgeBase.settings.overview.attention', { count: totals.attention })}
          </span>
        )}
      </div>

      {overview && entries.length === 0 && (
        <p className="text-sm text-ink-muted">{t('knowledgeBase.settings.storageEmpty')}</p>
      )}

      {entries.length > 0 && (
        <div className="overflow-x-auto border border-subtle rounded-container">
          <table className="w-full text-xs">
            <thead className="bg-surface-2 text-ink-muted">
              <tr className="text-left">
                <th className="font-medium whitespace-nowrap px-3 py-2">
                  {t('knowledgeBase.settings.overview.colKnowledgeBase')}
                </th>
                <th className="font-medium whitespace-nowrap px-3 py-2 text-right">
                  {t('knowledgeBase.stats.passages')}
                </th>
                <th className="font-medium whitespace-nowrap px-3 py-2 text-right">
                  {t('knowledgeBase.stats.storage')}
                </th>
                <th className="font-medium whitespace-nowrap px-3 py-2">
                  {t('knowledgeBase.settings.overview.colSearches', { days: totals.days })}
                </th>
                <th className="font-medium whitespace-nowrap px-3 py-2 text-right">
                  {t('knowledgeBase.settings.overview.colSearchTime')}
                </th>
                <th className="font-medium whitespace-nowrap px-3 py-2">
                  {t('knowledgeBase.settings.overview.colLastUsed')}
                </th>
                <th className="font-medium whitespace-nowrap px-3 py-2">
                  {t('knowledgeBase.settings.overview.colHealth')}
                </th>
                <th className="px-3 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-subtle tabular-nums">
              {entries.map((e) => {
                const s = e.stats
                const key = `${e.agentId}/${e.kbId}`
                return (
                  <tr key={key} className="align-top hover:bg-surface-2">
                    <td className="px-3 py-2 min-w-[12rem]">
                      <div className="text-sm font-medium text-ink">{e.name}</div>
                      <div className="text-ink-muted">
                        {e.orphan ? (
                          <span className="text-warning">{t('knowledgeBase.settings.orphan')}</span>
                        ) : (
                          agentName(e.agentId)
                        )}
                        {e.embedding ? ` · ${embeddingLabel(e.embedding)}` : ''}
                      </div>
                    </td>
                    <td className="px-3 py-2 text-right text-ink">
                      {compactNumber(e.chunkCount)}
                      <div className="text-ink-muted">
                        {t('knowledgeBase.settings.overview.sources', { count: e.sourceCount })}
                      </div>
                    </td>
                    <td className="px-3 py-2 text-right text-ink min-w-[6rem]">
                      {formatBytes(e.bytes)}
                      {s && s.health.reclaimableBytes > 0 && (
                        <div className="mt-1">
                          <Meter
                            value={s.health.reclaimableBytes}
                            max={e.bytes}
                            tone={s.health.reclaimableBytes >= 0.2 * e.bytes ? 'warning' : 'accent'}
                            label={t('knowledgeBase.stats.reclaimable', {
                              size: formatBytes(s.health.reclaimableBytes)
                            })}
                          />
                        </div>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-2">
                        <span className="text-ink w-8 text-right">
                          {compactNumber(s?.usage.searchesInPeriod ?? 0)}
                        </span>
                        {s && (
                          <Sparkline
                            values={s.usage.dailySearches}
                            width={88}
                            height={24}
                            label={t('knowledgeBase.stats.searchesPerDay', {
                              days: s.periodDays
                            })}
                          />
                        )}
                      </div>
                    </td>
                    <td className="px-3 py-2 text-right text-ink whitespace-nowrap">
                      {formatDuration(s?.usage.latencyP50Ms)}
                      {s?.usage.latencyP95Ms != null && (
                        <div className="text-ink-muted">
                          {t('knowledgeBase.settings.overview.p95Short', {
                            time: formatDuration(s.usage.latencyP95Ms)
                          })}
                        </div>
                      )}
                    </td>
                    <td className="px-3 py-2 text-ink-muted whitespace-nowrap">
                      {relativeTime(s?.usage.lastSearchAt, i18n.language)}
                    </td>
                    <td className="px-3 py-2">
                      {s ? (
                        <span
                          title={s.health.issues
                            .map((i) =>
                              t(`knowledgeBase.health.issue.${i.kind}`, {
                                count: i.count,
                                size: formatBytes(i.count)
                              })
                            )
                            .join('\n')}
                        >
                          <HealthChip issues={s.health.issues} />
                        </span>
                      ) : (
                        '—'
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex gap-1 justify-end">
                        <button
                          type="button"
                          className={buttonClass}
                          disabled={busy !== null}
                          title={t('knowledgeBase.maintenance.optimize')}
                          onClick={async () => {
                            const r = await optimize(e)
                            if (r?.integrity === 'ok')
                              toast.success(
                                t('knowledgeBase.maintenance.optimized', {
                                  before: formatBytes(r.bytesBefore),
                                  after: formatBytes(r.bytesAfter)
                                })
                              )
                            refresh()
                          }}
                        >
                          {busy === key ? (
                            <FiRefreshCw className="w-3 h-3 animate-spin" />
                          ) : (
                            <FiTool className="w-3 h-3" />
                          )}
                        </button>
                        <button
                          type="button"
                          className={buttonClass}
                          title={t('knowledgeBase.delete')}
                          onClick={() => deleteKb(e)}
                        >
                          <FiTrash2 className="w-3 h-3 text-danger" />
                        </button>
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </SettingSection>
  )
}

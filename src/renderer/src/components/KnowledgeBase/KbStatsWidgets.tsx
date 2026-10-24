import React, { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiAlertOctagon, FiAlertTriangle, FiCheckCircle, FiInfo } from 'react-icons/fi'
import type { KbStats, KnowledgeHealthIssue } from '@/types/knowledge'
import { formatBytes } from './knowledgeSettings'

// ---------------------------------------------------------------------------------------------
// formatting
// ---------------------------------------------------------------------------------------------

/** 1,284 / 12.9K / 4.2M. */
export const compactNumber = (n: number): string =>
  new Intl.NumberFormat(undefined, { notation: 'compact', maximumFractionDigits: 1 }).format(n)

/** 840 ms / 2.4 s / 3 min 5 s. */
export function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return '—'
  if (ms < 1000) return `${Math.round(ms)} ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)} s`
  const m = Math.floor(ms / 60_000)
  const s = Math.round((ms % 60_000) / 1000)
  return s ? `${m} min ${s} s` : `${m} min`
}

/** "3 hours ago" in the UI language. */
export function relativeTime(at: number | string | null | undefined, locale?: string): string {
  if (at == null) return '—'
  const t = typeof at === 'number' ? at : Date.parse(at)
  if (!Number.isFinite(t)) return '—'
  const diff = (t - Date.now()) / 1000
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
  const steps: [Intl.RelativeTimeFormatUnit, number][] = [
    ['second', 60],
    ['minute', 60],
    ['hour', 24],
    ['day', 30],
    ['month', 12],
    ['year', Infinity]
  ]
  let v = diff
  for (const [unit, size] of steps) {
    if (Math.abs(v) < size) return rtf.format(Math.round(v), unit)
    v /= size
  }
  return rtf.format(Math.round(v), 'year')
}

// ---------------------------------------------------------------------------------------------
// sparkline
// ---------------------------------------------------------------------------------------------

/**
 * Daily counts as a 2px accent line over a 10% wash, the last point marked. Hovering (or focusing)
 * shows the day and its count; the value is also in the tile beside it, so nothing is hover-only.
 */
export const Sparkline: React.FC<{
  values: number[]
  /** Accessible summary, e.g. "Searches per day, last 30 days". */
  label: string
  width?: number
  height?: number
}> = ({ values, label, width = 120, height = 32 }) => {
  const { t, i18n } = useTranslation()
  const [hover, setHover] = useState<number | null>(null)
  const gradientId = useId()
  // An all-zero series is a flat line that reads like a control, not data.
  if (values.length === 0 || values.every((v) => v === 0)) return null
  const pad = 4
  const max = Math.max(1, ...values)
  const step = values.length > 1 ? (width - pad * 2) / (values.length - 1) : 0
  const x = (i: number) => pad + i * step
  const y = (v: number) => height - pad - (v / max) * (height - pad * 2)
  const line = values
    .map((v, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(v).toFixed(1)}`)
    .join('')
  const area = `${line}L${x(values.length - 1).toFixed(1)},${height - pad}L${pad},${height - pad}Z`
  const last = values.length - 1
  const day = (i: number) => {
    const d = new Date()
    d.setDate(d.getDate() - (last - i))
    return d.toLocaleDateString(i18n.language, { month: 'short', day: 'numeric' })
  }
  const shown = hover

  return (
    <div className="relative inline-block" onMouseLeave={() => setHover(null)}>
      <svg
        width={width}
        height={height}
        role="img"
        aria-label={label}
        className="block text-accent overflow-visible"
      >
        <defs>
          <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
            <stop offset="0" stopColor="currentColor" stopOpacity="0.14" />
            <stop offset="1" stopColor="currentColor" stopOpacity="0.02" />
          </linearGradient>
        </defs>
        <line
          x1={pad}
          x2={width - pad}
          y1={height - pad}
          y2={height - pad}
          stroke="var(--border)"
          strokeWidth="1"
        />
        <path d={area} fill={`url(#${gradientId})`} />
        <path
          d={line}
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinejoin="round"
          strokeLinecap="round"
        />
        {shown !== null && (
          <line
            x1={x(shown)}
            x2={x(shown)}
            y1={pad}
            y2={height - pad}
            stroke="currentColor"
            strokeOpacity="0.35"
            strokeWidth="1"
          />
        )}
        <circle
          cx={x(shown ?? last)}
          cy={y(values[shown ?? last])}
          r="4"
          fill="currentColor"
          stroke="var(--surface)"
          strokeWidth="2"
        />
        {values.map((_, i) => (
          <rect
            key={i}
            x={x(i) - step / 2}
            y={0}
            width={Math.max(step, 6)}
            height={height}
            fill="transparent"
            onMouseEnter={() => setHover(i)}
          />
        ))}
      </svg>
      {shown !== null && (
        <div
          className="absolute z-20 -top-8 px-2 py-1 rounded-control bg-raised border border-subtle shadow-sm text-xs whitespace-nowrap pointer-events-none"
          style={{ left: Math.min(Math.max(x(shown) - 40, 0), width - 60) }}
        >
          <span className="font-semibold text-ink">{values[shown]}</span>{' '}
          <span className="text-ink-muted">
            {t('knowledgeBase.stats.searchesOn', { day: day(shown) })}
          </span>
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------------------------
// tiles, meter, health
// ---------------------------------------------------------------------------------------------

export const StatTile: React.FC<{
  label: string
  value: React.ReactNode
  sub?: React.ReactNode
  children?: React.ReactNode
  title?: string
}> = ({ label, value, sub, children, title }) => (
  <div
    className="min-w-0 rounded-container border border-subtle bg-surface px-3 py-2.5 flex flex-col gap-0.5"
    title={title}
  >
    <div className="text-xs text-ink-muted truncate">{label}</div>
    <div className="text-lg font-semibold text-ink leading-tight truncate">{value}</div>
    {sub && <div className="text-xs text-ink-muted truncate">{sub}</div>}
    {children && <div className="mt-1">{children}</div>}
  </div>
)

/**
 * Share of a whole as a thin bar on a lighter track of the same hue. `tone` picks the hue: accent
 * for neutral fill, warning when the share is worth acting on.
 */
export const Meter: React.FC<{
  value: number
  max: number
  label: string
  tone?: 'accent' | 'warning'
}> = ({ value, max, label, tone = 'accent' }) => {
  const pct = max > 0 ? Math.min(100, Math.max(0, (value / max) * 100)) : 0
  const fill = tone === 'warning' ? 'bg-warning' : 'bg-accent'
  const track = tone === 'warning' ? 'bg-warning-soft' : 'bg-accent-tint'
  return (
    <div
      className={`h-1.5 w-full rounded-full overflow-hidden ${track}`}
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={value}
      title={label}
    >
      <div className={`h-full rounded-full ${fill}`} style={{ width: `${pct}%` }} />
    </div>
  )
}

const severityOrder: Record<KnowledgeHealthIssue['severity'], number> = {
  critical: 0,
  warning: 1,
  info: 2
}

export const sortedIssues = (issues: KnowledgeHealthIssue[]) =>
  [...issues].sort((a, b) => severityOrder[a.severity] - severityOrder[b.severity])

/** Healthy / Needs attention / Problem, with an icon so the state never rests on color alone. */
export const HealthChip: React.FC<{ issues: KnowledgeHealthIssue[] }> = ({ issues }) => {
  const { t } = useTranslation()
  const worst = sortedIssues(issues.filter((i) => i.kind !== 'empty'))[0]
  const [Icon, cls, label] = !worst
    ? [FiCheckCircle, 'text-success bg-success-soft', t('knowledgeBase.health.healthy')]
    : worst.severity === 'critical'
      ? [FiAlertOctagon, 'text-danger bg-danger-soft', t('knowledgeBase.health.problem')]
      : worst.severity === 'warning'
        ? [FiAlertTriangle, 'text-warning bg-warning-soft', t('knowledgeBase.health.attention')]
        : [FiInfo, 'text-accent bg-accent-tint', t('knowledgeBase.health.suggestion')]
  return (
    <span
      className={`inline-flex items-center gap-1 text-xs font-medium px-1.5 py-0.5 rounded-control whitespace-nowrap ${cls}`}
    >
      <Icon className="w-3 h-3" aria-hidden />
      {label}
    </span>
  )
}

/** One line per issue, worst first. */
export const HealthList: React.FC<{ issues: KnowledgeHealthIssue[] }> = ({ issues }) => {
  const { t } = useTranslation()
  if (issues.length === 0) return null
  return (
    <ul className="space-y-1">
      {sortedIssues(issues).map((i) => {
        const Icon =
          i.severity === 'critical'
            ? FiAlertOctagon
            : i.severity === 'warning'
              ? FiAlertTriangle
              : FiInfo
        const cls =
          i.severity === 'critical'
            ? 'text-danger'
            : i.severity === 'warning'
              ? 'text-warning'
              : 'text-accent'
        return (
          <li key={i.kind} className="flex items-start gap-1.5 text-xs text-ink">
            <Icon className={`w-3.5 h-3.5 mt-px flex-shrink-0 ${cls}`} aria-hidden />
            <span>
              {t(`knowledgeBase.health.issue.${i.kind}`, {
                count: i.count,
                size: formatBytes(i.count)
              })}
            </span>
          </li>
        )
      })}
    </ul>
  )
}

/** The KPI row for one knowledgebase. */
export const KbStatTiles: React.FC<{ stats: KbStats }> = ({ stats }) => {
  const { t, i18n } = useTranslation()
  const { info, usage, health } = stats
  const reclaimShare = info.bytes > 0 ? health.reclaimableBytes / info.bytes : 0
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-2">
      <StatTile
        label={t('knowledgeBase.stats.passages')}
        value={compactNumber(info.chunkCount)}
        sub={t('knowledgeBase.stats.sourcesBreakdown', {
          documents: compactNumber(info.documentCount),
          files: stats.fileSources,
          websites: stats.websiteSources
        })}
      />
      <StatTile
        label={t('knowledgeBase.stats.storage')}
        value={formatBytes(info.bytes)}
        sub={
          health.reclaimableBytes > 0
            ? t('knowledgeBase.stats.reclaimable', { size: formatBytes(health.reclaimableBytes) })
            : t('knowledgeBase.stats.compact')
        }
      >
        <Meter
          value={health.reclaimableBytes}
          max={info.bytes}
          tone={reclaimShare >= 0.2 ? 'warning' : 'accent'}
          label={t('knowledgeBase.stats.reclaimable', {
            size: formatBytes(health.reclaimableBytes)
          })}
        />
      </StatTile>
      <StatTile
        label={t('knowledgeBase.stats.searches', { days: stats.periodDays })}
        value={compactNumber(usage.searchesInPeriod)}
        sub={
          usage.lastSearchAt
            ? t('knowledgeBase.stats.lastUsed', {
                time: relativeTime(usage.lastSearchAt, i18n.language)
              })
            : t('knowledgeBase.stats.neverUsed')
        }
      >
        <Sparkline
          values={usage.dailySearches}
          label={t('knowledgeBase.stats.searchesPerDay', { days: stats.periodDays })}
        />
      </StatTile>
      <StatTile
        label={t('knowledgeBase.stats.searchTime')}
        value={formatDuration(usage.latencyP50Ms)}
        sub={
          usage.latencyP95Ms != null
            ? t('knowledgeBase.stats.p95', { time: formatDuration(usage.latencyP95Ms) })
            : t('knowledgeBase.stats.noSearches')
        }
        title={t('knowledgeBase.stats.searchTimeHint')}
      />
    </div>
  )
}

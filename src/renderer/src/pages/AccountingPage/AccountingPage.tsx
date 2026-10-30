import React, { useCallback, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import toast from 'react-hot-toast'
import { FiDownload, FiX } from 'react-icons/fi'
import type { UsageEventRow, UsageSummaryRow } from '@/types/usage'
import { Badge, Button, Card } from '@renderer/components/ui'
import {
  byCost,
  formatTokens,
  formatUsd,
  monthOf,
  pickerMonths,
  share,
  unitAmounts
} from './accountingUtils'
import { DailyChart, MonthlyChart } from './UsageCharts'
import { BudgetCard } from './BudgetCard'
import { useBudgetStatus } from './useBudgetStatus'
import useSetting from '@renderer/hooks/useSetting'
import { DEFAULT_AGENTS } from '@renderer/pages/ChatPage/constants/DEFAULT_AGENTS'
import { useUsageReport, type UsageFilter } from './useUsageReport'

const Tile: React.FC<{ label: string; value: string; hint?: string }> = ({
  label,
  value,
  hint
}) => (
  <Card className="flex flex-col gap-1 min-w-[10rem]">
    <span className="text-xs text-ink-muted">{label}</span>
    <span className="text-xl font-semibold text-ink tabular-nums">{value}</span>
    {hint && <span className="text-xs text-ink-muted">{hint}</span>}
  </Card>
)

type Column<R> = { label: string; right?: boolean; cell: (r: R) => React.ReactNode }

function Table<R>({
  title,
  hint,
  rows,
  rowKey,
  columns,
  selected,
  onRowClick
}: {
  title: string
  hint?: string
  rows: R[]
  rowKey: (r: R) => string | number
  columns: Column<R>[]
  selected?: string
  onRowClick?: (r: R) => void
}) {
  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-baseline gap-2">
        <h2 className="text-heading font-medium text-ink">{title}</h2>
        {hint && <span className="text-xs text-ink-muted">{hint}</span>}
      </div>
      <div className="overflow-x-auto border border-subtle rounded-container">
        <table className="w-full text-xs">
          <thead className="bg-surface-2 text-ink-muted">
            <tr className="text-left">
              {columns.map((c) => (
                <th
                  key={c.label}
                  className={`font-medium whitespace-nowrap px-3 py-2 ${c.right ? 'text-right' : ''}`}
                >
                  {c.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-subtle tabular-nums">
            {rows.map((r) => {
              const key = rowKey(r)
              return (
                <tr
                  key={key}
                  onClick={onRowClick ? () => onRowClick(r) : undefined}
                  className={`hover:bg-surface-2 ${onRowClick ? 'cursor-pointer' : ''} ${
                    selected === key ? 'bg-accent-tint' : ''
                  }`}
                >
                  {columns.map((c) => (
                    <td
                      key={c.label}
                      className={`px-3 py-1.5 whitespace-nowrap ${c.right ? 'text-right' : ''}`}
                    >
                      {c.cell(r)}
                    </td>
                  ))}
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </section>
  )
}

/**
 * Accounting: estimated Bedrock cost of every model call the app made, by month, day, model and
 * feature. The data is the usage ledger in the Rust backend (`window.api.usage`). Clicking a
 * model or feature narrows the whole page to it.
 */
export const AccountingPage: React.FC = () => {
  const { t, i18n } = useTranslation()
  const current = monthOf(new Date())
  const [month, setMonth] = useState(current)
  const [filter, setFilter] = useState<UsageFilter>({})
  const { report, months, info, events, moreEvents, loadMoreEvents, error } = useUsageReport(
    month,
    filter
  )
  const [exporting, setExporting] = useState(false)
  const budget = useBudgetStatus()
  const { agents } = useSetting()
  const agentName = useCallback(
    (id: string) => {
      if (id === '') return t('accounting.noAgent')
      // Chat sessions are saved with this placeholder, so imported history can't name the agent.
      if (id === 'defaultAgent') return t('accounting.unrecordedAgent')
      return (
        agents.find((a) => a.id === id)?.name ?? DEFAULT_AGENTS.find((a) => a.id === id)?.name ?? id
      )
    },
    [agents, t]
  )

  const monthLabel = (m: string) => {
    const [y, mo] = m.split('-').map(Number)
    return new Date(y, mo - 1, 1).toLocaleDateString(i18n.language, {
      year: 'numeric',
      month: 'long'
    })
  }

  const featureLabel = useCallback(
    (key: string) =>
      key.startsWith('tool:')
        ? t('accounting.feature.tool', { name: key.slice(5) })
        : t(`accounting.feature.${key}`, { defaultValue: key }),
    [t]
  )

  const modelName = (id: string) => report?.allModels.find((m) => m.key === id)?.label ?? id

  const total = report?.total
  const totalCost = total?.costUsd ?? 0

  /** "2 images · 12 s of video" for the non-token services; empty for tokens only. */
  const unitsText = useCallback(
    (pairs: [string, number][]) =>
      pairs
        .map(([kind, n]) => t(`accounting.units.${kind}`, { count: n, value: formatTokens(n) }))
        .join(' · '),
    [t]
  )

  const countColumns: Column<UsageSummaryRow>[] = useMemo(
    () => [
      { label: t('accounting.col.calls'), right: true, cell: (r) => formatTokens(r.calls) },
      {
        label: t('accounting.col.input'),
        right: true,
        cell: (r) => formatTokens(r.inputTokens)
      },
      {
        label: t('accounting.col.output'),
        right: true,
        cell: (r) => formatTokens(r.outputTokens)
      },
      {
        label: t('accounting.col.cache'),
        right: true,
        cell: (r) => `${formatTokens(r.cacheReadTokens)} / ${formatTokens(r.cacheWriteTokens)}`
      },
      {
        label: t('accounting.col.units'),
        right: true,
        cell: (r) => unitsText(unitAmounts(r))
      },
      {
        label: t('accounting.col.cost'),
        right: true,
        cell: (r) => (
          <span
            title={r.unpricedCalls ? t('accounting.unpricedRow', { count: r.unpricedCalls }) : ''}
          >
            {formatUsd(r.costUsd)}
            {r.unpricedCalls > 0 && <span className="text-warning"> *</span>}
          </span>
        )
      },
      { label: t('accounting.col.share'), right: true, cell: (r) => share(r.costUsd, totalCost) }
    ],
    [t, totalCost, unitsText]
  )

  const eventColumns: Column<UsageEventRow>[] = useMemo(
    () => [
      {
        label: t('accounting.col.time'),
        cell: (r) =>
          new Date(r.ts).toLocaleString(i18n.language, {
            month: 'short',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit'
          })
      },
      {
        label: t('accounting.col.model'),
        cell: (r) => <span title={r.rawModelId}>{r.modelName}</span>
      },
      {
        label: t('accounting.col.feature'),
        cell: (r) => (
          <span className="inline-flex items-center gap-1">
            {featureLabel(r.feature)}
            {r.source === 'backfill' && <Badge>{t('accounting.importedBadge')}</Badge>}
            {r.calls > 1 && (
              <span title={t('accounting.batchTitle', { count: r.calls })}>
                <Badge>{t('accounting.batchBadge', { count: r.calls })}</Badge>
              </span>
            )}
          </span>
        )
      },
      {
        label: t('accounting.col.input'),
        right: true,
        cell: (r) => (
          <span title={r.estimated ? t('accounting.estimatedTokens') : ''}>
            {r.estimated ? '~' : ''}
            {formatTokens(r.inputTokens)}
          </span>
        )
      },
      {
        label: t('accounting.col.output'),
        right: true,
        cell: (r) => formatTokens(r.outputTokens)
      },
      {
        label: t('accounting.col.cache'),
        right: true,
        cell: (r) => `${formatTokens(r.cacheReadTokens)} / ${formatTokens(r.cacheWriteTokens)}`
      },
      {
        label: t('accounting.col.units'),
        right: true,
        cell: (r) => (r.unitKind && r.units ? unitsText([[r.unitKind, r.units]]) : '')
      },
      {
        label: t('accounting.col.cost'),
        right: true,
        cell: (r) =>
          r.costUsd === null ? (
            <span className="text-ink-muted">{t('accounting.noPrice')}</span>
          ) : (
            formatUsd(r.costUsd)
          )
      }
    ],
    [t, i18n.language, featureLabel, unitsText]
  )

  const toggle = (key: keyof UsageFilter, value: string) =>
    setFilter((f) => ({ ...f, [key]: f[key] === value ? undefined : value }))

  const exportCsv = async () => {
    if (!report) return
    setExporting(true)
    try {
      const name = ['bedrock-usage', month, filter.modelId, filter.feature]
        .filter(Boolean)
        .join('-')
        .replace(/[^\w.-]+/g, '_')
      const path = await window.api.usage.exportCsv(report.query, `${name}.csv`)
      if (path) toast.success(t('accounting.exported', { path }))
    } catch (e) {
      toast.error(t('accounting.error', { error: e instanceof Error ? e.message : String(e) }))
    } finally {
      setExporting(false)
    }
  }

  const imported = total?.importedCalls ?? 0
  const filtered = Boolean(filter.modelId || filter.feature || filter.agentId !== undefined)

  return (
    <div className="flex flex-col h-full px-2.5 py-1.5 gap-4 overflow-y-auto">
      <header className="flex flex-col gap-1">
        <div className="flex items-center gap-4 flex-wrap">
          <h1 className="text-title text-ink">{t('accounting.title')}</h1>
          <select
            aria-label={t('accounting.month')}
            className="rounded-control border border-subtle bg-surface text-sm text-ink px-2 py-1"
            value={month}
            onChange={(e) => setMonth(e.target.value)}
          >
            {pickerMonths(months, current).map((m) => (
              <option key={m} value={m}>
                {monthLabel(m)}
              </option>
            ))}
          </select>
          <Button
            size="sm"
            onClick={exportCsv}
            disabled={!report || report.days.length === 0 || exporting}
          >
            <FiDownload aria-hidden />
            {t('accounting.exportCsv')}
          </Button>
        </div>
        <p className="text-ink-muted">{t('accounting.description')}</p>
      </header>

      {filtered && (
        <div className="flex items-center gap-2 flex-wrap text-sm">
          <span className="text-ink-muted">{t('accounting.filteredBy')}</span>
          {filter.modelId && (
            <Button size="sm" variant="subtle" onClick={() => toggle('modelId', filter.modelId!)}>
              {modelName(filter.modelId)}
              <FiX aria-label={t('accounting.clearFilter')} />
            </Button>
          )}
          {filter.feature && (
            <Button size="sm" variant="subtle" onClick={() => toggle('feature', filter.feature!)}>
              {featureLabel(filter.feature)}
              <FiX aria-label={t('accounting.clearFilter')} />
            </Button>
          )}
          {filter.agentId !== undefined && (
            <Button size="sm" variant="subtle" onClick={() => toggle('agentId', filter.agentId!)}>
              {agentName(filter.agentId)}
              <FiX aria-label={t('accounting.clearFilter')} />
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={() => setFilter({})}>
            {t('accounting.clearAll')}
          </Button>
        </div>
      )}

      {error && <p className="text-sm text-danger">{t('accounting.error', { error })}</p>}

      {report && (
        <>
          <div className="flex flex-wrap gap-2.5">
            <Tile
              label={month === current ? t('accounting.monthToDate') : t('accounting.monthTotal')}
              value={formatUsd(totalCost)}
            />
            {month === current && budget && !filtered && (
              <Tile
                label={t('accounting.projected')}
                value={formatUsd(budget.projectedUsd)}
                hint={t('accounting.projectedHint')}
              />
            )}
            <Tile label={t('accounting.lastMonth')} value={formatUsd(report.lastMonthCost)} />
            <Tile label={t('accounting.calls')} value={formatTokens(total?.calls ?? 0)} />
            {(total?.cacheReadTokens ?? 0) > 0 && (
              <Tile
                label={t('accounting.cacheSavings')}
                value={formatUsd(total?.cacheSavingsUsd ?? 0)}
                hint={t('accounting.cacheSavingsHint')}
              />
            )}
            <Tile
              label={t('accounting.unpriced')}
              value={formatTokens(total?.unpricedCalls ?? 0)}
              hint={t('accounting.unpricedHint')}
            />
          </div>

          {month === current && <BudgetCard status={budget} />}

          {imported > 0 && info && (
            <p className="text-xs text-ink-muted">
              {t('accounting.importedNote', {
                count: imported,
                date: new Date(info.liveSince).toLocaleDateString(i18n.language, {
                  year: 'numeric',
                  month: 'long',
                  day: 'numeric'
                })
              })}
            </p>
          )}

          <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
            <DailyChart month={month} report={report} />
            <MonthlyChart month={month} report={report} onSelect={setMonth} />
          </div>

          {report.days.length === 0 ? (
            <p className="text-sm text-ink-muted">{t('accounting.empty')}</p>
          ) : (
            <>
              <Table
                title={t('accounting.byModel')}
                hint={t('accounting.clickToFilter')}
                rows={byCost(report.models)}
                rowKey={(r) => r.key}
                selected={filter.modelId}
                onRowClick={(r) => toggle('modelId', r.key)}
                columns={[
                  {
                    label: t('accounting.col.model'),
                    cell: (r) => <span title={r.key}>{r.label}</span>
                  },
                  ...countColumns
                ]}
              />
              <Table
                title={t('accounting.byFeature')}
                hint={t('accounting.clickToFilter')}
                rows={byCost(report.features)}
                rowKey={(r) => r.key}
                selected={filter.feature}
                onRowClick={(r) => toggle('feature', r.key)}
                columns={[
                  { label: t('accounting.col.feature'), cell: (r) => featureLabel(r.key) },
                  ...countColumns
                ]}
              />
              <Table
                title={t('accounting.byAgent')}
                hint={t('accounting.clickToFilter')}
                rows={byCost(report.agents)}
                rowKey={(r) => r.key}
                selected={filter.agentId}
                onRowClick={(r) => toggle('agentId', r.key)}
                columns={[
                  { label: t('accounting.col.agent'), cell: (r) => agentName(r.key) },
                  ...countColumns
                ]}
              />
              <Table
                title={t('accounting.byDay')}
                rows={[...report.days].reverse()}
                rowKey={(r) => r.key}
                columns={[
                  {
                    label: t('accounting.col.day'),
                    cell: (r) =>
                      new Date(`${r.key}T12:00:00`).toLocaleDateString(i18n.language, {
                        month: 'short',
                        day: 'numeric',
                        weekday: 'short'
                      })
                  },
                  ...countColumns
                ]}
              />
              <Table
                title={t('accounting.calls')}
                rows={events}
                rowKey={(r) => r.id}
                columns={eventColumns}
              />
              {moreEvents && (
                <div>
                  <Button size="sm" onClick={() => loadMoreEvents()}>
                    {t('accounting.loadMore')}
                  </Button>
                </div>
              )}
            </>
          )}
          <p className="text-xs text-ink-muted">{t('accounting.footnote')}</p>
        </>
      )}
    </div>
  )
}

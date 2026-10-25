import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  UsageBreakdownRow,
  UsageEventRow,
  UsageInfo,
  UsageQuery,
  UsageSummaryRow
} from '@/types/usage'
import { monthRange, previousMonth, totals, type UsageTotals } from './accountingUtils'

/** Narrow the report to one model, feature and/or agent (`''` = no agent). */
export type UsageFilter = { modelId?: string; feature?: string; agentId?: string }

export type UsageReport = {
  /** The selected month, filtered. */
  query: UsageQuery
  days: UsageSummaryRow[]
  dailyByModel: UsageBreakdownRow[]
  /** The month's models (feature filter only, so every model stays pickable). */
  models: UsageSummaryRow[]
  /** The month's models with no filter: decides each model's color. */
  allModels: UsageSummaryRow[]
  /** The month's features (the other filters only). */
  features: UsageSummaryRow[]
  /** The month's agents (the other filters only). */
  agents: UsageSummaryRow[]
  /** Every month on record, filtered. */
  monthly: UsageSummaryRow[]
  total: UsageTotals
  lastMonthCost: number
}

const EVENTS_PAGE = 50
const ALL_TIME = { from: '0000-01-01', to: '9999-12-31' }

/** The selected month's report, refreshed whenever new usage is recorded. */
export function useUsageReport(month: string, filter: UsageFilter) {
  const [report, setReport] = useState<UsageReport | null>(null)
  const [months, setMonths] = useState<string[]>([])
  const [info, setInfo] = useState<UsageInfo | null>(null)
  const [events, setEvents] = useState<UsageEventRow[]>([])
  const [moreEvents, setMoreEvents] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const { modelId, feature, agentId } = filter
  // Ignore responses for a selection that has since changed.
  const selection = `${month}|${modelId ?? ''}|${feature ?? ''}|${agentId ?? '-'}`
  const latest = useRef(selection)
  latest.current = selection

  const load = useCallback(async () => {
    const usage = window.api.usage
    const range = monthRange(month)
    const query: UsageQuery = { ...range, modelId, feature, agentId }
    try {
      const [
        days,
        daily,
        models,
        allModels,
        features,
        monthly,
        last,
        withData,
        ledgerInfo,
        recent,
        agents
      ] = await Promise.all([
        usage.summary(query, 'day'),
        usage.breakdown(query, 'day', 'model'),
        usage.summary({ ...range, feature, agentId }, 'model'),
        usage.summary(range, 'model'),
        usage.summary({ ...range, modelId, agentId }, 'feature'),
        usage.summary({ ...ALL_TIME, modelId, feature, agentId }, 'month'),
        usage.summary({ ...monthRange(previousMonth(month)), modelId, feature, agentId }, 'month'),
        usage.months(),
        usage.info(),
        usage.events(query, EVENTS_PAGE, 0),
        usage.summary({ ...range, modelId, feature }, 'agent')
      ])
      if (latest.current !== selection) return
      setReport({
        query,
        days,
        dailyByModel: daily,
        models,
        allModels,
        features,
        agents,
        monthly,
        total: totals(days),
        lastMonthCost: totals(last).costUsd
      })
      setMonths(withData)
      setInfo(ledgerInfo)
      setEvents(recent)
      setMoreEvents(recent.length === EVENTS_PAGE)
      setError(null)
    } catch (e) {
      if (latest.current === selection) setError(e instanceof Error ? e.message : String(e))
    }
  }, [month, modelId, feature, agentId, selection])

  const loadMoreEvents = useCallback(async () => {
    if (!report) return
    const next = await window.api.usage.events(report.query, EVENTS_PAGE, events.length)
    if (latest.current !== selection) return
    setEvents((prev) => [...prev, ...next])
    setMoreEvents(next.length === EVENTS_PAGE)
  }, [report, events.length, selection])

  useEffect(() => {
    load()
    return window.api.usage.onChanged(() => {
      load()
    })
  }, [load])

  return { report, months, info, events, moreEvents, loadMoreEvents, error }
}

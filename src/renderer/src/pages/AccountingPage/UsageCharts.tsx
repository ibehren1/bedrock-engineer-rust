import React, { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Bar } from 'react-chartjs-2'
import {
  Chart as ChartJS,
  BarElement,
  CategoryScale,
  LinearScale,
  Legend,
  Tooltip,
  type ChartOptions,
  type ChartDataset,
  type ScriptableContext
} from 'chart.js'
import { isDarkAppearance } from '@renderer/lib/appearance'
import { formatUsd } from './accountingUtils'
import { dailyByModel, modelSlots, monthDays, monthSpan, monthlyCost } from './chartData'
import type { UsageReport } from './useUsageReport'

ChartJS.register(BarElement, CategoryScale, LinearScale, Legend, Tooltip)

// Categorical slots in fixed order (validated for light and dark surfaces); "Other" is neutral.
const SERIES_LIGHT = ['#2a78d6', '#eb6834', '#1baf7a', '#eda100', '#e87ba4', '#008300']
const SERIES_DARK = ['#3987e5', '#d95926', '#199e70', '#c98500', '#d55181', '#008300']
const OTHER_LIGHT = '#a3a29d'
const OTHER_DARK = '#6b6a65'

/** Theme colors from the app's CSS tokens, so the charts follow the selected appearance. */
function themeColors() {
  const css = getComputedStyle(document.documentElement)
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback
  const dark = isDarkAppearance()
  return {
    dark,
    surface: v('--surface', dark ? '#1a1a19' : '#fcfcfb'),
    ink: v('--ink-muted', dark ? '#c3c2b7' : '#52514e'),
    grid: v('--border-soft', dark ? '#2c2c2a' : '#ebeae6')
  }
}

function baseOptions(colors: ReturnType<typeof themeColors>): ChartOptions<'bar'> {
  return {
    responsive: true,
    maintainAspectRatio: false,
    animation: false,
    interaction: { mode: 'index', intersect: false },
    plugins: {
      legend: {
        position: 'bottom',
        labels: {
          color: colors.ink,
          boxWidth: 10,
          boxHeight: 10,
          useBorderRadius: true,
          borderRadius: 2
        }
      },
      tooltip: {
        callbacks: {
          label: (ctx) => ` ${ctx.dataset.label ?? ''}: ${formatUsd(Number(ctx.raw) || 0)}`
        },
        filter: (item) => Number(item.raw) > 0
      }
    },
    scales: {
      x: { stacked: true, grid: { display: false }, ticks: { color: colors.ink, maxRotation: 0 } },
      y: {
        stacked: true,
        beginAtZero: true,
        border: { display: false },
        grid: { color: colors.grid },
        ticks: { color: colors.ink, callback: (v) => formatUsd(Number(v)) }
      }
    }
  }
}

const ChartCard: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <section className="flex flex-col gap-2">
    <h2 className="text-heading font-medium text-ink">{title}</h2>
    <div className="h-64 rounded-container border border-subtle bg-surface p-2.5">{children}</div>
  </section>
)

/** Daily cost stacked by model for the selected month. */
export const DailyChart: React.FC<{ month: string; report: UsageReport }> = ({ month, report }) => {
  const { t, i18n } = useTranslation()
  const colors = themeColors()
  const data = useMemo(() => {
    const days = monthDays(month)
    const series = dailyByModel(
      days,
      report.dailyByModel,
      modelSlots(report.allModels),
      t('accounting.otherModels')
    )
    const palette = colors.dark ? SERIES_DARK : SERIES_LIGHT
    // Only the top segment of each day's stack gets rounded corners.
    const top = days.map((_, i) => series.findLastIndex((s) => s.data[i] > 0))
    return {
      labels: days.map((d) => String(Number(d.slice(8)))),
      datasets: series.map((s, si): ChartDataset<'bar', number[]> => ({
        label: s.label,
        data: s.data,
        backgroundColor:
          s.slot === null ? (colors.dark ? OTHER_DARK : OTHER_LIGHT) : palette[s.slot],
        // A surface-colored top edge leaves a 2px gap between stacked segments.
        borderColor: colors.surface,
        borderWidth: { top: 2, left: 0, right: 0, bottom: 0 },
        borderRadius: (ctx: ScriptableContext<'bar'>) =>
          top[ctx.dataIndex] === si
            ? { topLeft: 4, topRight: 4, bottomLeft: 0, bottomRight: 0 }
            : 0,
        borderSkipped: false,
        maxBarThickness: 18
      }))
    }
  }, [month, report, t, colors.dark, colors.surface])

  const options = useMemo(() => {
    const o = baseOptions(colors)
    o.plugins!.tooltip!.callbacks!.title = (items) => {
      const day = `${month}-${String(items[0]?.label ?? '').padStart(2, '0')}`
      return new Date(`${day}T12:00:00`).toLocaleDateString(i18n.language, {
        weekday: 'short',
        month: 'short',
        day: 'numeric'
      })
    }
    return o
  }, [colors.dark, colors.surface, month, i18n.language])

  return (
    <ChartCard title={t('accounting.dailyChart')}>
      <Bar data={data} options={options} aria-label={t('accounting.dailyChart')} role="img" />
    </ChartCard>
  )
}

/** Cost per month over everything on record, the selected month highlighted. */
export const MonthlyChart: React.FC<{
  month: string
  report: UsageReport
  onSelect: (month: string) => void
}> = ({ month, report, onSelect }) => {
  const { t, i18n } = useTranslation()
  const colors = themeColors()
  const months = useMemo(() => {
    const keys = report.monthly.map((r) => r.key)
    const first = keys[0] ?? month
    const last = [keys[keys.length - 1] ?? month, month].sort()[1]
    return monthSpan(first < month ? first : month, last)
  }, [report.monthly, month])

  const label = (m: string) => {
    const [y, mo] = m.split('-').map(Number)
    return new Date(y, mo - 1, 1).toLocaleDateString(i18n.language, {
      month: 'short',
      year: '2-digit'
    })
  }

  const data = useMemo(() => {
    const base = colors.dark ? SERIES_DARK[0] : SERIES_LIGHT[0]
    return {
      labels: months.map(label),
      datasets: [
        {
          label: t('accounting.col.cost'),
          data: monthlyCost(months, report.monthly),
          // The selected month in full color, the others lighter.
          backgroundColor: months.map((m) => (m === month ? base : `${base}66`)),
          borderRadius: { topLeft: 4, topRight: 4 },
          borderSkipped: false as const,
          maxBarThickness: 32
        }
      ]
    }
  }, [months, report.monthly, month, colors.dark, colors.surface, t, i18n.language])

  const options = useMemo(() => {
    const o = baseOptions(colors)
    o.plugins!.legend = { display: false }
    o.onClick = (_e, elements) => {
      const i = elements[0]?.index
      if (i !== undefined) onSelect(months[i])
    }
    o.onHover = (e, elements) => {
      const target = e.native?.target as HTMLElement | undefined
      if (target) target.style.cursor = elements.length ? 'pointer' : 'default'
    }
    return o
  }, [colors.dark, colors.surface, months, onSelect])

  return (
    <ChartCard title={t('accounting.monthlyChart')}>
      <Bar data={data} options={options} aria-label={t('accounting.monthlyChart')} role="img" />
    </ChartCard>
  )
}

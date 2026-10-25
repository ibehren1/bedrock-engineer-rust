import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import toast from 'react-hot-toast'
import { FiAlertTriangle } from 'react-icons/fi'
import type { UsageBudgetStatus } from '@/types/usage'
import { Button, Card } from '@renderer/components/ui'
import { formatUsd } from './accountingUtils'
import { readBudget, saveBudget } from './useBudgetStatus'

/** "80, 100" -> [0.8, 1]; `null` if anything isn't a positive number. */
export function parseThresholds(text: string): number[] | null {
  const parts = text
    .split(/[,\s]+/)
    .map((p) => p.replace('%', '').trim())
    .filter(Boolean)
  if (parts.length === 0) return null
  const values = parts.map(Number)
  if (values.some((v) => !Number.isFinite(v) || v <= 0)) return null
  return [...new Set(values.map((v) => v / 100))].sort((a, b) => a - b)
}

export const formatThresholds = (alertAt: number[]) =>
  alertAt.map((f) => `${Math.round(f * 100)}`).join(', ')

/** The monthly budget: progress for this month, and the editor. */
export const BudgetCard: React.FC<{ status: UsageBudgetStatus | null }> = ({ status }) => {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const [amount, setAmount] = useState('')
  const [thresholds, setThresholds] = useState('')

  const startEditing = () => {
    const b = readBudget()
    setAmount(b.monthlyUsd ? String(b.monthlyUsd) : '')
    setThresholds(formatThresholds(b.alertAt))
    setEditing(true)
  }

  const save = async () => {
    const monthlyUsd = Number(amount)
    const alertAt = parseThresholds(thresholds)
    if (!Number.isFinite(monthlyUsd) || monthlyUsd <= 0 || !alertAt) {
      toast.error(t('accounting.budget.invalid'))
      return
    }
    await saveBudget({ monthlyUsd, alertAt })
    setEditing(false)
  }

  const remove = async () => {
    await saveBudget({ ...readBudget(), monthlyUsd: null })
    setEditing(false)
  }

  const fraction = status?.fraction ?? null
  const tone =
    fraction === null
      ? 'bg-accent'
      : fraction >= 1
        ? 'bg-danger'
        : fraction >= Math.min(...readBudget().alertAt)
          ? 'bg-warning'
          : 'bg-accent'

  return (
    <Card className="flex flex-col gap-2 max-w-xl">
      <div className="flex items-center gap-2">
        <span className="text-sm font-medium text-ink">{t('accounting.budget.title')}</span>
        {!editing && (
          <Button size="sm" variant="ghost" onClick={startEditing}>
            {status?.budgetUsd ? t('accounting.budget.edit') : t('accounting.budget.set')}
          </Button>
        )}
      </div>

      {editing ? (
        <form
          className="flex flex-wrap items-end gap-2 text-sm"
          onSubmit={(e) => {
            e.preventDefault()
            save()
          }}
        >
          <label className="flex flex-col gap-1">
            <span className="text-xs text-ink-muted">{t('accounting.budget.amount')}</span>
            <input
              type="number"
              min="0"
              step="any"
              inputMode="decimal"
              className="w-28 rounded-control border border-subtle bg-surface px-2 py-1 text-ink"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              autoFocus
            />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-xs text-ink-muted">{t('accounting.budget.alertAt')}</span>
            <input
              className="w-28 rounded-control border border-subtle bg-surface px-2 py-1 text-ink"
              value={thresholds}
              onChange={(e) => setThresholds(e.target.value)}
            />
          </label>
          <Button size="sm" variant="primary" type="submit">
            {t('accounting.budget.save')}
          </Button>
          <Button size="sm" onClick={() => setEditing(false)}>
            {t('accounting.budget.cancel')}
          </Button>
          {status?.budgetUsd && (
            <Button size="sm" variant="ghost" onClick={remove}>
              {t('accounting.budget.remove')}
            </Button>
          )}
        </form>
      ) : status?.budgetUsd ? (
        <>
          <div
            className="h-2 rounded-full bg-surface-2 overflow-hidden"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round((fraction ?? 0) * 100)}
          >
            <div
              className={`h-full rounded-full ${tone}`}
              style={{ width: `${Math.min(100, (fraction ?? 0) * 100)}%` }}
            />
          </div>
          <div className="flex items-center gap-1.5 text-xs text-ink-muted">
            {status.overBudget && <FiAlertTriangle className="text-danger" aria-hidden />}
            <span>
              {t(status.overBudget ? 'accounting.budget.over' : 'accounting.budget.progress', {
                spent: formatUsd(status.spentUsd),
                budget: formatUsd(status.budgetUsd),
                percent: Math.round((fraction ?? 0) * 100)
              })}
            </span>
          </div>
          <span className="text-xs text-ink-muted">
            {t('accounting.budget.alerts', { thresholds: formatThresholds(readBudget().alertAt) })}
          </span>
        </>
      ) : (
        <span className="text-xs text-ink-muted">{t('accounting.budget.none')}</span>
      )}
    </Card>
  )
}

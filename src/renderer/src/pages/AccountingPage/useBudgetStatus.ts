import { useCallback, useEffect, useState } from 'react'
import type { UsageBudget, UsageBudgetStatus } from '@/types/usage'
import { storeFlush } from '@renderer/lib/tauriBridge'

/** Fired on `window` after the budget setting is saved, so every status reader refreshes. */
export const BUDGET_CHANGED_EVENT = 'usage-budget-changed'

export const DEFAULT_BUDGET: UsageBudget = { monthlyUsd: null, alertAt: [0.8, 1] }

export function readBudget(): UsageBudget {
  return { ...DEFAULT_BUDGET, ...(window.store.get('usageBudget') ?? {}) }
}

export async function saveBudget(budget: UsageBudget) {
  window.store.set('usageBudget', budget)
  // The status is computed in Rust from the stored setting, so wait for the write.
  await storeFlush()
  window.dispatchEvent(new Event(BUDGET_CHANGED_EVENT))
}

/** This month's spend against the budget; refreshed on new usage and budget changes. */
export function useBudgetStatus() {
  const [status, setStatus] = useState<UsageBudgetStatus | null>(null)

  const load = useCallback(() => {
    window.api.usage
      .budgetStatus()
      .then(setStatus)
      .catch(() => setStatus(null))
  }, [])

  useEffect(() => {
    load()
    const off = window.api.usage.onChanged(load)
    window.addEventListener(BUDGET_CHANGED_EVENT, load)
    return () => {
      off()
      window.removeEventListener(BUDGET_CHANGED_EVENT, load)
    }
  }, [load])

  return status
}

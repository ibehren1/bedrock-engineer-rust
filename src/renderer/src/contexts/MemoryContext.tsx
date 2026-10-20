import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import type { MemoryAddParams, MemoryItem, MemoryUpdateParams } from '@/types/bridge/memory'

interface MemoryContextType {
  /** All memory items, newest first. */
  items: MemoryItem[]
  /** Number of enabled items (the ones that go into the system prompt). */
  enabledCount: number
  loading: boolean
  add: (params: MemoryAddParams) => Promise<MemoryItem>
  update: (params: MemoryUpdateParams) => Promise<MemoryItem>
  remove: (id: string) => Promise<boolean>
  refresh: () => Promise<void>
}

const MemoryContext = createContext<MemoryContextType | undefined>(undefined)

const upsert = (items: MemoryItem[], item: MemoryItem): MemoryItem[] =>
  items.some((i) => i.id === item.id)
    ? items.map((i) => (i.id === item.id ? item : i))
    : [item, ...items]

/**
 * The global memory items list. Loaded once and kept current through `memory-changed`, which Rust
 * fires after every write (this window's, another window's, or a model tool call).
 */
export const MemoryProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [items, setItems] = useState<MemoryItem[]>([])
  const [loading, setLoading] = useState(true)

  const refresh = useCallback(async () => {
    try {
      setItems(await window.memory.list())
    } catch (error) {
      console.error('Failed to load memory items:', error)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    const unlisten = window.memory.onChanged((next) => {
      if (Array.isArray(next)) setItems(next)
      setLoading(false)
    })
    refresh()
    return unlisten
  }, [refresh])

  // Apply each write's result right away too, so the UI doesn't wait for the event.
  const add = useCallback(async (params: MemoryAddParams) => {
    const item = await window.memory.add(params)
    setItems((prev) => upsert(prev, item))
    return item
  }, [])

  const update = useCallback(async (params: MemoryUpdateParams) => {
    const item = await window.memory.update(params)
    setItems((prev) => upsert(prev, item))
    return item
  }, [])

  const remove = useCallback(async (id: string) => {
    const removed = await window.memory.delete(id)
    setItems((prev) => prev.filter((i) => i.id !== id))
    return removed
  }, [])

  const value = useMemo(
    () => ({
      items,
      enabledCount: items.filter((i) => i.enabled).length,
      loading,
      add,
      update,
      remove,
      refresh
    }),
    [items, loading, add, update, remove, refresh]
  )

  return <MemoryContext.Provider value={value}>{children}</MemoryContext.Provider>
}

/** Like `useMemoryItems`, but `undefined` outside a `MemoryProvider` instead of throwing. */
export const useOptionalMemoryItems = (): MemoryContextType | undefined => useContext(MemoryContext)

export const useMemoryItems = (): MemoryContextType => {
  const context = useContext(MemoryContext)
  if (context === undefined) {
    throw new Error('useMemoryItems must be used within a MemoryProvider')
  }
  return context
}

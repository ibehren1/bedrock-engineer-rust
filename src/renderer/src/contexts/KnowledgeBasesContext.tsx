import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState
} from 'react'
import toast from 'react-hot-toast'
import { useTranslation } from 'react-i18next'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { KbInfo } from '@/types/knowledge'
import { useSettings } from './SettingsContext'
import { reconcileKnowledgeRefs, setKnowledgeRef } from './knowledgeRefs'

interface KnowledgeBasesContextType {
  /** Every local knowledgebase, as the backend lists them. */
  knowledgeBases: KbInfo[]
  /** Until the first load finishes. */
  loading: boolean
  /** Reload the list; resolves to it, or undefined when loading failed. */
  refresh: () => Promise<KbInfo[] | undefined>
  /** Create a knowledgebase; shows the error (a duplicate name, say) and resolves undefined. */
  create: (name: string, description: string) => Promise<KbInfo | undefined>
  /** Rename or re-describe a knowledgebase; shows the error and resolves undefined. */
  update: (kbId: string, name: string, description: string) => Promise<KbInfo | undefined>
  /** Delete a knowledgebase; every agent that used it loses it. Resolves whether it worked. */
  remove: (kbId: string) => Promise<boolean>
  /** Attach `kbId` to the custom agent `agentId`, or detach it, and save the agent list. */
  setAttached: (agentId: string, kbId: string, on: boolean) => void
}

const KnowledgeBasesContext = createContext<KnowledgeBasesContextType | undefined>(undefined)

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e))

/**
 * The central list of local knowledgebases, kept current through `knowledge:changed`.
 *
 * Agents attach knowledgebases by id (`customAgents[].localKnowledgeBases`). After every
 * successful load, and whenever the agents change, the refs are reconciled with the list: refs to
 * deleted knowledgebases go, names and descriptions follow renames. Never after a failed load,
 * which would detach everything. Only the main window loads (the commands are granted to it
 * alone); other windows get an empty list.
 */
export const KnowledgeBasesProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const { t } = useTranslation()
  const { customAgents, saveCustomAgents } = useSettings()
  const [knowledgeBases, setKnowledgeBases] = useState<KbInfo[]>([])
  const [loading, setLoading] = useState(true)
  // Whether the list in state came from a successful load; reconciling trusts nothing else.
  const [trusted, setTrusted] = useState(false)
  const isMain = useMemo(() => getCurrentWindow().label === 'main', [])

  const latest = useRef({ customAgents, saveCustomAgents, knowledgeBases })
  latest.current = { customAgents, saveCustomAgents, knowledgeBases }

  const refresh = useCallback(async (): Promise<KbInfo[] | undefined> => {
    if (!isMain) {
      setLoading(false)
      return undefined
    }
    try {
      const list = await window.api.knowledge.list()
      setKnowledgeBases(list)
      setTrusted(true)
      return list
    } catch (e) {
      setTrusted(false)
      console.error('Failed to list knowledge bases:', e)
      return undefined
    } finally {
      setLoading(false)
    }
  }, [isMain])

  useEffect(() => {
    refresh()
    if (!isMain) return undefined
    // Coalesce bursts of change events (a crawl emits many).
    let pending: ReturnType<typeof setTimeout> | null = null
    const off = window.api.knowledge.onChanged(() => {
      if (pending) clearTimeout(pending)
      pending = setTimeout(() => {
        pending = null
        refresh()
      }, 200)
    })
    return () => {
      off()
      if (pending) clearTimeout(pending)
    }
  }, [isMain, refresh])

  // Keep the agents' refs true to the list.
  useEffect(() => {
    if (!isMain || !trusted) return
    const next = reconcileKnowledgeRefs(customAgents, knowledgeBases)
    if (next) saveCustomAgents(next)
    // saveCustomAgents is a new function every render; the inputs are what matter.
  }, [isMain, trusted, customAgents, knowledgeBases])

  const create = useCallback(
    async (name: string, description: string) => {
      try {
        const kb = await window.api.knowledge.create(name, description)
        await refresh()
        return kb
      } catch (e) {
        toast.error(errorText(e))
        return undefined
      }
    },
    [refresh]
  )

  const update = useCallback(
    async (kbId: string, name: string, description: string) => {
      try {
        const kb = await window.api.knowledge.update(kbId, name, description)
        await refresh()
        return kb
      } catch (e) {
        toast.error(errorText(e))
        return undefined
      }
    },
    [refresh]
  )

  const remove = useCallback(
    async (kbId: string) => {
      try {
        await window.api.knowledge.delete(kbId)
        toast.success(t('knowledgeBase.deleted'))
        await refresh()
        return true
      } catch (e) {
        toast.error(errorText(e))
        return false
      }
    },
    [refresh, t]
  )

  const setAttached = useCallback((agentId: string, kbId: string, on: boolean) => {
    const { customAgents: agents, saveCustomAgents: save, knowledgeBases: kbs } = latest.current
    const kb = kbs.find((k) => k.id === kbId)
    const agent = agents.find((a) => a.id === agentId)
    if (!kb || !agent) return
    if (!!agent.localKnowledgeBases?.some((ref) => ref.id === kbId) === on) return
    const refs = setKnowledgeRef(agent.localKnowledgeBases, kb, on)
    save(agents.map((a) => (a.id === agentId ? { ...a, localKnowledgeBases: refs } : a)))
  }, [])

  const value = useMemo(
    () => ({ knowledgeBases, loading, refresh, create, update, remove, setAttached }),
    [knowledgeBases, loading, refresh, create, update, remove, setAttached]
  )

  return <KnowledgeBasesContext.Provider value={value}>{children}</KnowledgeBasesContext.Provider>
}

export const useKnowledgeBaseList = (): KnowledgeBasesContextType => {
  const context = useContext(KnowledgeBasesContext)
  if (!context) {
    throw new Error('useKnowledgeBaseList must be used within a KnowledgeBasesProvider')
  }
  return context
}

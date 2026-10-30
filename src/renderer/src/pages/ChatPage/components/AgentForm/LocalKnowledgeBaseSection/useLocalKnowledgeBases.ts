import { useCallback, useEffect, useRef, useState } from 'react'
import toast from 'react-hot-toast'
import { useTranslation } from 'react-i18next'
import type { LocalKnowledgeBaseRef } from '@/types/agent-chat'
import { formatBytes } from '@renderer/components/KnowledgeBase/knowledgeSettings'
import type { KbInfo, KnowledgeJob, SourceInfo, UrlOptions } from '@/types/knowledge'

const toRefs = (kbs: KbInfo[]): LocalKnowledgeBaseRef[] =>
  kbs.map(({ id, name, description }) => ({ id, name, description }))

const sameRefs = (a: LocalKnowledgeBaseRef[] = [], b: LocalKnowledgeBaseRef[] = []) =>
  JSON.stringify(a) === JSON.stringify(b)

/** Key of a job in `jobs`: its source, or the knowledgebase for a rebuild. */
export const jobKey = (job: Pick<KnowledgeJob, 'kbId' | 'sourceId'>) =>
  job.sourceId ?? `kb:${job.kbId}`

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e))

/**
 * An agent's local knowledgebases, kept in sync with the backend (`knowledge:changed` /
 * `knowledge:progress` events). `onRefsChange` gets the agent's `localKnowledgeBases` list whenever
 * the knowledgebases on disk change, so the agent always lists what exists.
 */
export function useLocalKnowledgeBases(
  agentId: string,
  refs: LocalKnowledgeBaseRef[] | undefined,
  onRefsChange: (refs: LocalKnowledgeBaseRef[]) => void
) {
  const { t } = useTranslation()
  const [kbs, setKbs] = useState<KbInfo[]>([])
  const [sources, setSources] = useState<Record<string, SourceInfo[]>>({})
  const [jobs, setJobs] = useState<Record<string, KnowledgeJob>>({})
  const [loading, setLoading] = useState(true)

  const refsRef = useRef(refs)
  refsRef.current = refs
  const onRefsChangeRef = useRef(onRefsChange)
  onRefsChangeRef.current = onRefsChange
  const kbsRef = useRef(kbs)
  kbsRef.current = kbs
  const sourcesRef = useRef(sources)
  sourcesRef.current = sources

  const refreshSources = useCallback(
    async (kbId: string) => {
      try {
        const list = await window.api.knowledge.listSources(agentId, kbId)
        setSources((prev) => ({ ...prev, [kbId]: list }))
      } catch (e) {
        // The knowledgebase may just have been deleted.
        setSources((prev) => {
          const next = { ...prev }
          delete next[kbId]
          return next
        })
        console.warn('Failed to list knowledge base sources:', e)
      }
    },
    [agentId]
  )

  const refresh = useCallback(async () => {
    try {
      const list = await window.api.knowledge.list(agentId)
      setKbs(list)
      const nextRefs = toRefs(list)
      if (!sameRefs(nextRefs, refsRef.current)) onRefsChangeRef.current(nextRefs)
      await Promise.all(list.map((kb) => refreshSources(kb.id)))
    } catch (e) {
      console.error('Failed to list knowledge bases:', e)
    } finally {
      setLoading(false)
    }
  }, [agentId, refreshSources])

  useEffect(() => {
    setLoading(true)
    refresh()
    window.api.knowledge
      .jobs(agentId)
      .then((list) => setJobs(Object.fromEntries(list.map((j) => [jobKey(j), j]))))
      .catch(() => {})
  }, [agentId, refresh])

  // Coalesce bursts of change events (a crawl emits many).
  const pending = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => {
    const offChanged = window.api.knowledge.onChanged((change) => {
      if (change.agentId !== agentId) return
      if (pending.current) clearTimeout(pending.current)
      pending.current = setTimeout(() => {
        pending.current = null
        refresh()
      }, 200)
    })
    const offProgress = window.api.knowledge.onProgress((job) => {
      if (job.agentId !== agentId) return
      const key = jobKey(job)
      if (job.phase === 'done' || job.phase === 'error' || job.phase === 'cancelled') {
        setJobs((prev) => {
          const next = { ...prev }
          delete next[key]
          return next
        })
        const kbName = kbsRef.current.find((k) => k.id === job.kbId)?.name ?? ''
        const source = sourcesRef.current[job.kbId]?.find((s) => s.id === job.sourceId)
        const name = source?.title ?? kbName
        if (job.phase === 'done') {
          toast.success(t('knowledgeBase.jobDone', { name, message: job.message }))
        } else if (job.phase === 'error') {
          toast.error(t('knowledgeBase.jobFailed', { name, message: job.message }), {
            duration: 8000
          })
        }
      } else {
        setJobs((prev) => ({ ...prev, [key]: job }))
      }
    })
    return () => {
      offChanged()
      offProgress()
      if (pending.current) clearTimeout(pending.current)
    }
  }, [agentId, refresh, t])

  /** Run an action, showing its error as a toast; resolves to its result or undefined. */
  const run = useCallback(async <T>(action: () => Promise<T>): Promise<T | undefined> => {
    try {
      return await action()
    } catch (e) {
      toast.error(errorText(e))
      return undefined
    }
  }, [])

  const k = window.api.knowledge
  return {
    kbs,
    sources,
    jobs,
    loading,
    refresh,
    create: (name: string, description: string) =>
      run(async () => {
        const kb = await k.create(agentId, name, description)
        await refresh()
        return kb
      }),
    update: (kbId: string, name: string, description: string) =>
      run(async () => {
        await k.update(agentId, kbId, name, description)
        await refresh()
      }),
    remove: (kbId: string) =>
      run(async () => {
        await k.delete(agentId, kbId)
        toast.success(t('knowledgeBase.deleted'))
        await refresh()
      }),
    rebuild: (kbId: string) => run(() => k.rebuild(agentId, kbId)),
    pickFiles: (kbId: string) =>
      run(async () => {
        const added = await k.pickFiles(agentId, kbId)
        if (added.length) await refreshSources(kbId)
        return added
      }),
    pickFolder: (kbId: string) =>
      run(async () => {
        const added = await k.pickFolder(agentId, kbId)
        if (added) await refreshSources(kbId)
        return added
      }),
    addUrl: (kbId: string, url: string, options: Parameters<typeof k.addUrl>[3]) =>
      run(async () => {
        const added = await k.addUrl(agentId, kbId, url, options)
        await refreshSources(kbId)
        return added
      }),
    resync: (kbId: string, sourceId: string) => run(() => k.resyncSource(agentId, kbId, sourceId)),
    updateSourceOptions: (kbId: string, sourceId: string, options: UrlOptions) =>
      run(async () => {
        const updated = await k.updateSourceOptions(agentId, kbId, sourceId, options)
        await refreshSources(kbId)
        return updated
      }),
    removeSource: (kbId: string, sourceId: string) =>
      run(async () => {
        await k.removeSource(agentId, kbId, sourceId)
        await refresh()
      }),
    cancelJob: (jobId: string) => run(() => k.cancelJob(jobId)),
    optimize: (kbId: string) =>
      run(async () => {
        const r = await k.optimize(agentId, kbId)
        if (r.integrity === 'ok') {
          toast.success(
            t('knowledgeBase.maintenance.optimized', {
              before: formatBytes(r.bytesBefore),
              after: formatBytes(r.bytesAfter)
            })
          )
        } else {
          toast.error(t('knowledgeBase.maintenance.integrityFailed', { result: r.integrity }))
        }
        await refresh()
        return r
      }),
    search: (query: string, kbId?: string) => k.search(agentId, query, kbId)
  }
}

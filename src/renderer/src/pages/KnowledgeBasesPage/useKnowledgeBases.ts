import { useCallback, useEffect, useRef, useState } from 'react'
import toast from 'react-hot-toast'
import { useTranslation } from 'react-i18next'
import { formatBytes } from '@renderer/components/KnowledgeBase/knowledgeSettings'
import { useKnowledgeBaseList } from '@renderer/contexts/KnowledgeBasesContext'
import type { KnowledgeJob, SourceInfo, UrlOptions } from '@/types/knowledge'

/** Key of a job in `jobs`: its source, or the knowledgebase for a rebuild. */
export const jobKey = (job: Pick<KnowledgeJob, 'kbId' | 'sourceId'>) =>
  job.sourceId ?? `kb:${job.kbId}`

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e))

/**
 * The Knowledge Bases page's view of every local knowledgebase: the central list
 * (`KnowledgeBasesContext`) plus each one's sources and running jobs, kept in sync with the
 * backend (`knowledge:changed` / `knowledge:progress`), and the actions on them.
 */
export function useKnowledgeBases() {
  const { t } = useTranslation()
  const list = useKnowledgeBaseList()
  const kbs = list.knowledgeBases
  const [sources, setSources] = useState<Record<string, SourceInfo[]>>({})
  const [jobs, setJobs] = useState<Record<string, KnowledgeJob>>({})

  const kbsRef = useRef(kbs)
  kbsRef.current = kbs
  const sourcesRef = useRef(sources)
  sourcesRef.current = sources

  const refreshSources = useCallback(async (kbId: string) => {
    try {
      const found = await window.api.knowledge.listSources(kbId)
      setSources((prev) => ({ ...prev, [kbId]: found }))
    } catch (e) {
      // The knowledgebase may just have been deleted.
      setSources((prev) => {
        const next = { ...prev }
        delete next[kbId]
        return next
      })
      console.warn('Failed to list knowledge base sources:', e)
    }
  }, [])

  // The list reloads on every change event; the sources follow it.
  useEffect(() => {
    kbs.forEach((kb) => refreshSources(kb.id))
  }, [kbs, refreshSources])

  useEffect(() => {
    window.api.knowledge
      .jobs()
      .then((found) => setJobs(Object.fromEntries(found.map((j) => [jobKey(j), j]))))
      .catch(() => {})
    return window.api.knowledge.onProgress((job) => {
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
  }, [t])

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
    loading: list.loading,
    refresh: list.refresh,
    create: list.create,
    update: list.update,
    remove: list.remove,
    rebuild: (kbId: string) => run(() => k.rebuild(kbId)),
    pickFiles: (kbId: string) =>
      run(async () => {
        const added = await k.pickFiles(kbId)
        if (added.length) await refreshSources(kbId)
        return added
      }),
    pickFolder: (kbId: string) =>
      run(async () => {
        const added = await k.pickFolder(kbId)
        if (added) await refreshSources(kbId)
        return added
      }),
    addUrl: (kbId: string, url: string, options: Parameters<typeof k.addUrl>[2]) =>
      run(async () => {
        const added = await k.addUrl(kbId, url, options)
        await refreshSources(kbId)
        return added
      }),
    resync: (kbId: string, sourceId: string) => run(() => k.resyncSource(kbId, sourceId)),
    updateSourceOptions: (kbId: string, sourceId: string, options: UrlOptions) =>
      run(async () => {
        const updated = await k.updateSourceOptions(kbId, sourceId, options)
        await refreshSources(kbId)
        return updated
      }),
    removeSource: (kbId: string, sourceId: string) =>
      run(async () => {
        await k.removeSource(kbId, sourceId)
        await list.refresh()
      }),
    cancelJob: (jobId: string) => run(() => k.cancelJob(jobId)),
    optimize: (kbId: string) =>
      run(async () => {
        const r = await k.optimize(kbId)
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
        await list.refresh()
        return r
      }),
    search: (query: string, kbId?: string) => k.search(query, kbId)
  }
}

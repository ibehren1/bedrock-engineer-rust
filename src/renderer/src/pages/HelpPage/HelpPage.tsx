import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useLocation } from 'react-router-dom'
import { FiMessageCircle, FiSearch } from 'react-icons/fi'
import type { HelpSearchResult } from '@/types/bridge/api'
import MD from '@renderer/components/Markdown/MD'
import { markdownHeadings } from '@renderer/components/Markdown/slug'
import { Button } from '@renderer/components/ui'
import useSetting from '@renderer/hooks/useSetting'
import { headingSlugForPath, snippet } from './helpPageUtils'
import { HelpChatPanel } from './HelpChatPanel'

/** Section 4 of the guide: setting up Bedrock access. */
const SETUP_ANCHOR = '4-setting-up-access-to-amazon-bedrock'

/**
 * Help: the user guide, readable with no AWS access or network, with a table of contents and
 * search over its shipped knowledge base. "Ask a question" opens the Help chat, which needs
 * Bedrock.
 */
export const HelpPage: React.FC = () => {
  const { t } = useTranslation()
  const location = useLocation()
  // `/help?ask=1` (the old Help chat entry point) opens with the chat panel showing.
  const [chatOpen, setChatOpen] = useState(() => new URLSearchParams(location.search).has('ask'))
  const { llmError, awsRegion, awsAccessKeyId, useAwsProfile } = useSetting()
  const [guide, setGuide] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [active, setActive] = useState<string>('')
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<HelpSearchResult | null>(null)
  const content = useRef<HTMLDivElement>(null)

  useEffect(() => {
    window.api.help
      .userGuide()
      .then(setGuide)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
  }, [])

  const headings = useMemo(() => (guide ? markdownHeadings(guide) : []), [guide])
  const toc = useMemo(() => headings.filter((h) => h.level === 2 || h.level === 3), [headings])

  const scrollTo = useCallback((id: string) => {
    const el = content.current?.querySelector(`#${CSS.escape(id)}`)
    el?.scrollIntoView({ block: 'start' })
    setActive(id)
  }, [])

  // Highlight the section being read.
  useEffect(() => {
    const box = content.current
    if (!box || toc.length === 0) return
    let frame = 0
    const onScroll = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        const top = box.getBoundingClientRect().top + 16
        let current = toc[0].slug
        for (const h of toc) {
          const el = box.querySelector(`#${CSS.escape(h.slug)}`)
          if (el && el.getBoundingClientRect().top <= top) current = h.slug
          else if (el) break
        }
        setActive(current)
      })
    }
    box.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      box.removeEventListener('scroll', onScroll)
      cancelAnimationFrame(frame)
    }
  }, [toc])

  // Search as you type (debounced).
  useEffect(() => {
    const q = query.trim()
    if (q.length < 2) {
      setResults(null)
      return
    }
    let live = true
    const timer = setTimeout(() => {
      window.api.help
        .search(q, 8)
        .then((r) => live && setResults(r))
        .catch(() => live && setResults({ mode: 'keyword', hits: [] }))
    }, 200)
    return () => {
      live = false
      clearTimeout(timer)
    }
  }, [query])

  const openHit = (headingPath: string) => {
    const slug = headingSlugForPath(headings, headingPath)
    if (slug) scrollTo(slug)
    setQuery('')
    setResults(null)
  }

  const noCredentials = useAwsProfile ? !awsRegion : !awsRegion || !awsAccessKeyId
  const bedrockMissing = Boolean(llmError) || noCredentials

  // Rendering 200 KB of Markdown is the expensive part: only when the text changes.
  const body = useMemo(
    () =>
      guide === null ? null : (
        <MD headingIds onAnchorClick={scrollTo}>
          {guide}
        </MD>
      ),
    [guide, scrollTo]
  )

  return (
    <div className="flex flex-col h-full px-2.5 py-1.5 gap-3">
      <header className="flex items-center gap-3 flex-wrap">
        <h1 className="text-title text-ink">{t('help.page.title')}</h1>
        <div className="relative flex-1 min-w-[14rem] max-w-xl">
          <FiSearch
            className="absolute left-2 top-1/2 -translate-y-1/2 text-ink-muted"
            aria-hidden
          />
          <input
            type="search"
            aria-label={t('help.page.search')}
            placeholder={t('help.page.search')}
            className="w-full rounded-control border border-subtle bg-surface pl-7 pr-2 py-1 text-sm text-ink"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && results?.hits[0]) openHit(results.hits[0].headingPath)
              if (e.key === 'Escape') setQuery('')
            }}
          />
          {results && (
            <ul className="absolute z-20 mt-1 w-full max-h-96 overflow-y-auto rounded-container border border-subtle bg-raised shadow-lg text-sm">
              {results.hits.length === 0 && (
                <li className="px-3 py-2 text-ink-muted">{t('help.page.noResults')}</li>
              )}
              {results.hits.map((hit, i) => (
                <li key={`${hit.headingPath}-${i}`}>
                  <button
                    type="button"
                    className="w-full text-left px-3 py-2 hover:bg-sunken focus:bg-sunken focus:outline-none"
                    onClick={() => openHit(hit.headingPath)}
                  >
                    <div className="font-medium text-ink truncate">
                      {hit.headingPath.split(' › ').slice(-2).join(' › ')}
                    </div>
                    <div className="text-xs text-ink-muted line-clamp-2">{snippet(hit.text)}</div>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant={chatOpen ? 'subtle' : 'primary'}
            onClick={() => setChatOpen((o) => !o)}
            title={t('help.page.askHint')}
            aria-pressed={chatOpen}
          >
            <FiMessageCircle aria-hidden />
            {t('help.page.ask')}
          </Button>
        </div>
      </header>

      {bedrockMissing && !chatOpen && (
        <p className="text-xs text-ink-muted">
          {t('help.page.needsBedrock')}{' '}
          <a
            href={`#${SETUP_ANCHOR}`}
            className="underline"
            onClick={(e) => {
              e.preventDefault()
              scrollTo(SETUP_ANCHOR)
            }}
          >
            {t('help.page.setUpAccess')}
          </a>
        </p>
      )}

      {error && <p className="text-sm text-danger">{t('help.page.loadFailed', { error })}</p>}

      <div className="flex flex-1 min-h-0 gap-4">
        <nav
          aria-label={t('help.page.contents')}
          className="hidden md:block w-64 shrink-0 overflow-y-auto text-sm border-r border-subtle pr-2"
        >
          <ul>
            {toc.map((h) => (
              <li key={h.slug}>
                <a
                  href={`#${h.slug}`}
                  onClick={(e) => {
                    e.preventDefault()
                    scrollTo(h.slug)
                  }}
                  className={`block rounded-control py-0.5 truncate ${
                    h.level === 3 ? 'pl-4 text-xs' : 'pl-1 font-medium'
                  } ${active === h.slug ? 'bg-accent-tint text-ink' : 'text-ink-muted hover:text-ink'}`}
                  title={h.text}
                >
                  {h.text}
                </a>
              </li>
            ))}
          </ul>
        </nav>
        <div ref={content} className="flex-1 min-w-0 overflow-y-auto pr-2 scroll-pt-2">
          {body}
        </div>
        {chatOpen && (
          <HelpChatPanel
            headings={headings}
            onOpenSection={scrollTo}
            onClose={() => setChatOpen(false)}
            bedrockMissing={bedrockMissing}
          />
        )}
      </div>
    </div>
  )
}

import { formatGuideExcerpts } from '@renderer/pages/ChatPage/constants/helpAgent'

/**
 * The Help chat's `prepareTurn` (see `useAgentChat`): the guide sections matching the question
 * and the one before it (so follow-ups work), from the guide's shipped knowledge base. With
 * `anchorFor`, each section carries its in-page anchor so answers can link to it. If the index
 * can't be searched, the whole guide goes along instead.
 */
export function makeHelpPrepareTurn(opts: {
  anchorFor?: (headingPath: string) => string | null
  onFallback?: () => void
}) {
  return async (input: string, earlier: string[]): Promise<string> => {
    const query = [earlier[earlier.length - 1], input].filter(Boolean).join('\n')
    try {
      const result = await window.api.help.search(query, 6)
      return formatGuideExcerpts(
        result.hits.map((h) => ({ ...h, anchor: opts.anchorFor?.(h.headingPath) ?? undefined }))
      )
    } catch {
      opts.onFallback?.()
      const guide = await window.api.help.userGuide()
      return `<user_guide>\n${guide}\n</user_guide>`
    }
  }
}

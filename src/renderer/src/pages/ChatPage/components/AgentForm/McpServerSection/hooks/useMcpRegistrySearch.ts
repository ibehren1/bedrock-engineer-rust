import { useCallback, useState } from 'react'
import { getStructuredOutput } from '@renderer/lib/api'
import { useLightProcessingModel } from '@renderer/lib/modelSelection'
import { McpRegistryServer } from '@/common/mcp/registry'
import { AgentMarketContext } from '../mcpMarket'
import {
  MCP_SEARCH_QUERY_TOOL,
  McpSearchQuery,
  McpSearchQueryResponse,
  buildMcpSearchQuerySystemPrompt,
  buildMcpSearchQueryUserMessage,
  mcpSearchQuerySchema
} from '../mcpSearchQueries'

export type RegistrySearchState = {
  /** Terms the results came from, with why each was chosen when the model picked it */
  terms: McpSearchQuery[]
  servers: McpRegistryServer[]
}

/** Keep the list scannable: per-term and overall caps on merged results. */
const RESULTS_PER_TERM = 5
const MAX_RESULTS = 20

/**
 * Search the official MCP Registry, either with terms the user typed or with
 * terms the model derives from the agent's configuration. Every server shown
 * comes from the registry — the model never names packages.
 */
export const useMcpRegistrySearch = () => {
  const { getLightModelId } = useLightProcessingModel()

  const [results, setResults] = useState<RegistrySearchState | null>(null)
  const [isSearching, setIsSearching] = useState(false)
  const [isSuggesting, setIsSuggesting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  /** Run one or more registry searches and merge the results, keeping order. */
  const runSearches = useCallback(async (terms: string[], existingServerNames: string[] = []) => {
    const configured = new Set(existingServerNames.map((name) => name.toLowerCase()))
    const merged = new Map<string, McpRegistryServer>()

    for (const term of terms) {
      const servers = await window.api.mcp.searchRegistry(term, RESULTS_PER_TERM)
      for (const server of servers) {
        if (merged.has(server.name)) continue
        if (configured.has(server.shortName.toLowerCase())) continue
        merged.set(server.name, server)
      }
      if (merged.size >= MAX_RESULTS) break
    }

    return [...merged.values()].slice(0, MAX_RESULTS)
  }, [])

  const search = useCallback(
    async (query: string, existingServerNames: string[] = []) => {
      const term = query.trim()
      if (!term) return

      setIsSearching(true)
      setError(null)
      try {
        setResults({
          terms: [{ term, reason: '' }],
          servers: await runSearches([term], existingServerNames)
        })
      } catch (e) {
        console.error('MCP registry search failed:', e)
        setError(e instanceof Error ? e.message : String(e))
        setResults(null)
      } finally {
        setIsSearching(false)
      }
    },
    [runSearches]
  )

  /** Ask the model for search terms, then search the registry with them. */
  const suggestAndSearch = useCallback(
    async (context: AgentMarketContext & { existingServerNames?: string[] }) => {
      setIsSuggesting(true)
      setError(null)

      try {
        const { queries } = await getStructuredOutput<McpSearchQueryResponse>({
          modelId: getLightModelId(),
          systemPrompt: buildMcpSearchQuerySystemPrompt(),
          userMessage: buildMcpSearchQueryUserMessage(context),
          outputSchema: mcpSearchQuerySchema,
          toolOptions: MCP_SEARCH_QUERY_TOOL,
          inferenceConfig: { maxTokens: 512, temperature: 0.3 }
        })

        const terms = (queries || [])
          .filter((query) => query?.term?.trim())
          .map((query) => ({ term: query.term.trim(), reason: query.reason?.trim() || '' }))
          .slice(0, 5)
        if (terms.length === 0) {
          setResults({ terms: [], servers: [] })
          return
        }

        setResults({
          terms,
          servers: await runSearches(
            terms.map((query) => query.term),
            context.existingServerNames || []
          )
        })
      } catch (e) {
        console.error('Failed to suggest MCP servers:', e)
        setError(e instanceof Error ? e.message : String(e))
        setResults(null)
      } finally {
        setIsSuggesting(false)
      }
    },
    [getLightModelId, runSearches]
  )

  const clear = useCallback(() => {
    setResults(null)
    setError(null)
  }, [])

  return { results, isSearching, isSuggesting, error, search, suggestAndSearch, clear }
}

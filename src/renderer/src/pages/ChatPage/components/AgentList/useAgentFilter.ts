import { useState, useMemo } from 'react'
import { CustomAgent } from '@/types/agent-chat'
import { HELP_AGENT_ID } from '../../constants/helpAgent'

export type SortKey = 'name' | 'description' | 'tags' | 'status' | null
export type SortOrder = 'asc' | 'desc'

/**
 * Special agents that back other pages and should never be offered as a chat
 * agent — neither in the agent list nor as an @mention delegation target.
 * `helpAgent` is here for the same reason: the sidebar's Help button is the only
 * way into it, and it is useless without the user guide that button attaches.
 */
export const EXCLUDED_CHAT_AGENT_IDS = [
  'reactGeneratorAgent',
  'vueGeneratorAgent',
  'svelteGeneratorAgent',
  // 'diagramGeneratorAgent',
  'softwareArchitectureAgent',
  'businessProcessAgent',
  HELP_AGENT_ID
]

/**
 * Default agents that can't be hidden. These are the agents no list shows in the
 * first place, so there is nothing to hide. `diagramGeneratorAgent` is not among
 * them: it is visible in the agent list and can be hidden, because
 * `getAgentTools` falls back to DEFAULT_AGENTS for the pages that resolve a
 * built-in agent by id (see DiagramGeneratorPage's diagramAgentId).
 */
export const PROTECTED_DEFAULT_AGENT_IDS = [...EXCLUDED_CHAT_AGENT_IDS]

export const useAgentFilter = (agents: CustomAgent[]) => {
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [sortKey, setSortKey] = useState<SortKey>(null)
  const [sortOrder, setSortOrder] = useState<SortOrder>('asc')

  const availableTags = useMemo(() => {
    const searchLower = searchQuery.toLowerCase()

    // Filter agents by search query (excluding selected tags filter)
    const searchFiltered = agents.filter((agent) => {
      // Don't display special agents used on other pages
      if (EXCLUDED_CHAT_AGENT_IDS.includes(agent.id)) return false

      // If no search query, include all agents
      if (searchQuery === '') return true

      // Search in name, description, and tags (matching AgentDirectory implementation)
      const nameMatch = agent.name.toLowerCase().includes(searchLower)
      const descMatch = agent.description?.toLowerCase().includes(searchLower) || false
      const tagMatch = agent.tags?.some((tag) => tag.toLowerCase().includes(searchLower)) || false

      return nameMatch || descMatch || tagMatch
    })

    // Extract tags from filtered agents
    const tagSet = new Set<string>()
    searchFiltered.forEach((agent) => {
      agent.tags?.forEach((tag) => tagSet.add(tag))
    })
    return Array.from(tagSet).sort()
  }, [agents, searchQuery])

  const filteredAgents = useMemo(() => {
    const searchLower = searchQuery.toLowerCase()

    // Filter agents
    const filtered = [...agents]
      .filter((agent) => {
        // Don't display special agents used on other pages
        return !EXCLUDED_CHAT_AGENT_IDS.includes(agent.id)
      })
      .filter((agent) => {
        // Search in name, description, and tags (matching AgentDirectory implementation)
        const nameMatch = agent.name.toLowerCase().includes(searchLower)
        const descMatch = agent.description?.toLowerCase().includes(searchLower) || false
        const tagMatch = agent.tags?.some((tag) => tag.toLowerCase().includes(searchLower)) || false

        const textMatch = nameMatch || descMatch || tagMatch

        // Selected tags filter
        const selectedTagsMatch =
          selectedTags.length === 0 || selectedTags.every((tag) => agent.tags?.includes(tag))

        return textMatch && selectedTagsMatch
      })

    // Sort agents
    if (!sortKey) {
      return filtered
    }

    return filtered.sort((a, b) => {
      let comparison = 0

      switch (sortKey) {
        case 'name':
          comparison = a.name.localeCompare(b.name)
          break
        case 'description':
          comparison = (a.description || '').localeCompare(b.description || '')
          break
        case 'tags': {
          // Sort by first tag
          const aFirstTag = a.tags?.[0] || ''
          const bFirstTag = b.tags?.[0] || ''
          comparison = aFirstTag.localeCompare(bFirstTag)
          break
        }
        case 'status': {
          // Sort by active/shared status
          const aStatus = a.isShared ? 'shared' : 'custom'
          const bStatus = b.isShared ? 'shared' : 'custom'
          comparison = aStatus.localeCompare(bStatus)
          break
        }
      }

      return sortOrder === 'asc' ? comparison : -comparison
    })
  }, [agents, searchQuery, selectedTags, sortKey, sortOrder])

  const toggleTag = (tag: string) => {
    setSelectedTags((prev) => (prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag]))
  }

  const handleSort = (key: SortKey) => {
    if (sortKey === key) {
      // Same key: toggle order or reset
      if (sortOrder === 'asc') {
        setSortOrder('desc')
      } else {
        // Reset sort
        setSortKey(null)
        setSortOrder('asc')
      }
    } else {
      // New key: set to ascending
      setSortKey(key)
      setSortOrder('asc')
    }
  }

  return {
    searchQuery,
    setSearchQuery,
    selectedTags,
    availableTags,
    filteredAgents,
    toggleTag,
    sortKey,
    sortOrder,
    handleSort
  }
}

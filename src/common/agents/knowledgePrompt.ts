import type { CustomAgent } from '../../types/agent-chat'

/**
 * The `<local_knowledge_bases>` block telling the model about the agent's local knowledgebases
 * and their search tool; '' for an agent without any. Same text as `knowledge_base_prompt` in
 * `src-tauri/crates/agents/src/prompt.rs` (background agents and sub-agents).
 */
export function buildKnowledgeBasePrompt(agent?: Pick<CustomAgent, 'localKnowledgeBases'>): string {
  const kbs = agent?.localKnowledgeBases ?? []
  if (kbs.length === 0) return ''
  const lines = [
    '<local_knowledge_bases>',
    'You have local knowledge bases with documents and websites the user added:',
    ...kbs.map((kb) => {
      const name = kb.name.trim()
      const description = (kb.description ?? '').trim()
      return description ? `- ${name}: ${description}` : `- ${name}`
    }),
    'Search them with the searchKnowledgeBase tool before answering questions they may cover, and cite the sources (file path or URL) of the passages you use.',
    '</local_knowledge_bases>'
  ]
  return lines.join('\n')
}

/** `basePrompt` with the knowledgebase block appended when the agent has local knowledgebases. */
export function withKnowledgeBasePrompt(
  basePrompt: string,
  agent?: Pick<CustomAgent, 'localKnowledgeBases'>
): string {
  const block = buildKnowledgeBasePrompt(agent)
  if (!block) return basePrompt
  return basePrompt ? `${basePrompt}\n\n${block}\n` : `${block}\n`
}

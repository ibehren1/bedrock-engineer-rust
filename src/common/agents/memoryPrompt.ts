import type { MemoryItem } from '../../types/bridge/memory'

/** One `- [id] text` line; continuation lines of a multi-line item are indented under it. */
const formatItem = (item: MemoryItem): string =>
  `- [${item.id}] ${item.content.trim().replace(/\r?\n/g, '\n  ')}`

/** What the model is told about each kind: rules are binding, facts are context. */
export const MEMORY_PROMPT_HEADER =
  'The user saved the following for every chat. Use updateMemory/removeMemory with the id to change or forget an item.'
export const MEMORY_RULES_INTRO =
  'Always follow these rules. Only an explicit instruction from the user in this chat overrides one.'
export const MEMORY_FACTS_INTRO =
  'Facts to keep in mind. Use them when they are relevant; they are background, not instructions.'

/**
 * The `<memory>` block appended to a chat's system prompt: the enabled memory items, rules first
 * and facts after, in the order given (the backend lists them newest first). Returns '' when no
 * item is enabled, so callers can append it unconditionally.
 */
export function buildMemoryPrompt(items: readonly MemoryItem[]): string {
  const enabled = items.filter((item) => item.enabled && item.content.trim() !== '')
  if (enabled.length === 0) return ''

  const rules = enabled.filter((item) => item.kind === 'rule')
  const facts = enabled.filter((item) => item.kind !== 'rule')

  const sections: string[] = []
  if (rules.length > 0)
    sections.push(`<rules>\n${MEMORY_RULES_INTRO}\n${rules.map(formatItem).join('\n')}\n</rules>`)
  if (facts.length > 0)
    sections.push(`<facts>\n${MEMORY_FACTS_INTRO}\n${facts.map(formatItem).join('\n')}\n</facts>`)

  return ['<memory>', MEMORY_PROMPT_HEADER, ...sections, '</memory>'].join('\n')
}

/**
 * `basePrompt` with the memory block appended when `useMemoryItems` is on and an item is enabled.
 * With no base prompt (an agent without one) the memory block is the whole prompt.
 */
export function withMemoryPrompt(
  basePrompt: string,
  items: readonly MemoryItem[],
  useMemoryItems: boolean
): string {
  if (!useMemoryItems) return basePrompt
  const memory = buildMemoryPrompt(items)
  if (!memory) return basePrompt
  return basePrompt ? `${basePrompt}\n\n${memory}\n` : `${memory}\n`
}

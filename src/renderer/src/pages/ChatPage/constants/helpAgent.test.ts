import { DEFAULT_AGENTS } from './DEFAULT_AGENTS'
import { formatGuideExcerpts, HELP_AGENT_ID, HELP_CHAT_TITLE } from './helpAgent'
import { previousUserPrompts } from '../hooks/useAgentChat'
import { EXCLUDED_CHAT_AGENT_IDS } from '../components/AgentList/useAgentFilter'

describe('the Help agent', () => {
  const helpAgent = DEFAULT_AGENTS.find((agent) => agent.id === HELP_AGENT_ID)

  it('ships as a default agent', () => {
    expect(helpAgent).toBeDefined()
  })

  // It answers from the user guide only (the sections sent with each question). A tool would let it claim to inspect the
  // user's machine, which it must never do.
  it('has no tools and no allowed commands', () => {
    expect(helpAgent?.tools).toEqual([])
    expect(helpAgent?.allowedCommands).toEqual([])
  })

  // The Help page's "Ask a question" is the only way in: the agent is useless without the guide
  // excerpts the Help chat adds, so it must not appear in the picker, agent list or @mention targets.
  it('is excluded from the chat agent lists', () => {
    expect(EXCLUDED_CHAT_AGENT_IDS).toContain(HELP_AGENT_ID)
  })

  // Titles starting with "Chat " are replaced by the auto-title generator.
  it('uses a chat title the auto-title generator will not overwrite', () => {
    expect(HELP_CHAT_TITLE.startsWith('Chat ')).toBe(false)
  })
})

describe('Help chat excerpts', () => {
  it('wraps each hit in a section with its heading path', () => {
    const out = formatGuideExcerpts([
      { headingPath: 'Guide › 26.4 A "monthly" budget', text: 'Set a budget.\n' }
    ])
    expect(out).toBe(
      '<guide_excerpts>\n<section path="Guide › 26.4 A &quot;monthly&quot; budget">\nSet a budget.\n</section>\n</guide_excerpts>'
    )
    expect(formatGuideExcerpts([])).toContain('No section of the guide matched')
  })

  it('collects earlier prompts but not tool results', () => {
    expect(
      previousUserPrompts([
        { role: 'user', content: [{ text: 'How do I set a budget?' }] },
        { role: 'assistant', content: [{ text: 'Open Accounting.' }] },
        {
          role: 'user',
          content: [{ toolResult: { toolUseId: 't', content: [{ text: 'x' }] } }]
        },
        { role: 'user', content: [{ guardContent: { text: { text: 'And alerts?' } } }] }
      ])
    ).toEqual(['How do I set a budget?', 'And alerts?'])
  })
})

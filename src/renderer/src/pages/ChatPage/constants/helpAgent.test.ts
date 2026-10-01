import { DEFAULT_AGENTS } from './DEFAULT_AGENTS'
import { HELP_AGENT_ID, HELP_CHAT_TITLE } from './helpAgent'
import { EXCLUDED_CHAT_AGENT_IDS } from '../components/AgentList/useAgentFilter'

describe('the Help agent', () => {
  const helpAgent = DEFAULT_AGENTS.find((agent) => agent.id === HELP_AGENT_ID)

  it('ships as a default agent', () => {
    expect(helpAgent).toBeDefined()
  })

  // It answers from the attached user guide only. A tool would let it claim to inspect the
  // user's machine, which it must never do.
  it('has no tools and no allowed commands', () => {
    expect(helpAgent?.tools).toEqual([])
    expect(helpAgent?.allowedCommands).toEqual([])
  })

  // The sidebar's Help button is the only way in: the agent is useless without the guide that
  // button attaches, so it must not appear in the picker, agent list or @mention targets.
  it('is excluded from the chat agent lists', () => {
    expect(EXCLUDED_CHAT_AGENT_IDS).toContain(HELP_AGENT_ID)
  })

  // Titles starting with "Chat " are replaced by the auto-title generator.
  it('uses a chat title the auto-title generator will not overwrite', () => {
    expect(HELP_CHAT_TITLE.startsWith('Chat ')).toBe(false)
  })
})

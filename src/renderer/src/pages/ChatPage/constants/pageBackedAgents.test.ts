import { DEFAULT_AGENTS } from './DEFAULT_AGENTS'

/**
 * The Diagram Generator and Website Generator pages resolve their agent by id and
 * fall back to DEFAULT_AGENTS when the user hides that agent (see getAgentTools in
 * SettingsContext). These ids therefore have to stay in DEFAULT_AGENTS, and they
 * have to keep declaring the tools those pages rely on.
 */
const PAGE_BACKED_AGENT_IDS = [
  'diagramGeneratorAgent',
  'softwareArchitectureAgent',
  'businessProcessAgent',
  'reactGeneratorAgent',
  'vueGeneratorAgent',
  'svelteGeneratorAgent'
]

describe('agents resolved by id from other pages', () => {
  it.each(PAGE_BACKED_AGENT_IDS)('%s exists in DEFAULT_AGENTS with tools', (agentId) => {
    const agent = DEFAULT_AGENTS.find((candidate) => candidate.id === agentId)

    expect(agent).toBeDefined()
    expect(agent?.tools?.length).toBeGreaterThan(0)
  })
})

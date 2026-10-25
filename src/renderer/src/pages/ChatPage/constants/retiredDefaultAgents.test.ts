import { adoptRetiredDefaultAgents } from './retiredDefaultAgents'
import { DEFAULT_AGENTS } from './DEFAULT_AGENTS'
import { CustomAgent } from '@/types/agent-chat'

const seededArchitect = {
  id: 'awsSolutionsArchitectAgent',
  name: 'AWS Solutions Architect',
  description: 'awsSolutionsArchitectAgent.description',
  system: 'prompt',
  scenarios: [{ title: 'Compare AWS services', content: '' }],
  isCustom: false
} as CustomAgent

describe('adoptRetiredDefaultAgents', () => {
  it('turns a seeded copy into an editable custom agent with literal text', () => {
    const { agents, changed } = adoptRetiredDefaultAgents([seededArchitect])
    expect(changed).toBe(true)
    expect(agents[0].isCustom).toBe(true)
    expect(agents[0].system).toBe('prompt')
    expect(agents[0].description).not.toContain('.description')
    for (const scenario of agents[0].scenarios) expect(scenario.content).not.toBe('')
  })

  it('leaves other agents and already adopted copies alone', () => {
    const adopted = { ...seededArchitect, isCustom: true, description: 'mine' }
    const { agents, changed } = adoptRetiredDefaultAgents([DEFAULT_AGENTS[0], adopted])
    expect(changed).toBe(false)
    expect(agents).toEqual([DEFAULT_AGENTS[0], adopted])
  })

  it('is not a default agent any more', () => {
    expect(DEFAULT_AGENTS.some((agent) => agent.id === seededArchitect.id)).toBe(false)
  })
})

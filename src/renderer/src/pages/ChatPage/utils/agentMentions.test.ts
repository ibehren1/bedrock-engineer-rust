import type { CustomAgent, ToolState } from '@/types/agent-chat'
import {
  applyDelegationAllowlist,
  getMentionableAgents,
  hasInvokeAgentTool,
  parseAgentMentions
} from './agentMentions'

const agent = (id: string, name: string, description?: string): CustomAgent =>
  ({ id, name, description, system: '', scenarios: [] }) as CustomAgent

const AGENTS: CustomAgent[] = [
  agent('reviewerAgent', 'Reviewer', 'Reviews code'),
  agent('reviewHelperAgent', 'Review Helper'),
  agent('writerAgent', 'Writer'),
  agent('writerProAgent', 'Writer Pro'),
  agent('architectAgent', 'Software Architect'),
  agent('callerAgent', 'Caller'),
  // Special agent that backs another page — never mentionable
  agent('reactGeneratorAgent', 'React Generator')
]

const invokeAgentTool = (): ToolState =>
  ({
    enabled: true,
    toolSpec: {
      name: 'invokeAgent',
      description: 'Delegate a task.',
      inputSchema: {
        json: {
          type: 'object',
          properties: { agentId: { type: 'string' }, task: { type: 'string' } }
        }
      }
    }
  }) as ToolState

const readFilesTool = (): ToolState =>
  ({
    enabled: true,
    toolSpec: {
      name: 'readFiles',
      description: 'Read files.',
      inputSchema: { json: { type: 'object', properties: {} } }
    }
  }) as ToolState

describe('getMentionableAgents', () => {
  it('excludes special page agents and the caller itself', () => {
    const ids = getMentionableAgents(AGENTS, 'callerAgent').map((a) => a.id)

    expect(ids).not.toContain('reactGeneratorAgent')
    expect(ids).not.toContain('callerAgent')
    expect(ids).toContain('reviewerAgent')
  })
})

describe('parseAgentMentions', () => {
  it('returns nothing when there is no @', () => {
    expect(parseAgentMentions('review src/main please', AGENTS)).toEqual([])
  })

  it('finds a single mention', () => {
    expect(parseAgentMentions('ask @Reviewer to look', AGENTS).map((a) => a.id)).toEqual([
      'reviewerAgent'
    ])
  })

  it('finds a mention at the very start of the message', () => {
    expect(parseAgentMentions('@Writer draft this', AGENTS).map((a) => a.id)).toEqual([
      'writerAgent'
    ])
  })

  it('finds a mention after an opening bracket', () => {
    expect(parseAgentMentions('see (@Writer) for this', AGENTS).map((a) => a.id)).toEqual([
      'writerAgent'
    ])
  })

  it('returns multiple mentions in first-mention order', () => {
    const ids = parseAgentMentions('first @Writer then @Reviewer', AGENTS).map((a) => a.id)
    expect(ids).toEqual(['writerAgent', 'reviewerAgent'])
  })

  it('dedupes a repeated mention', () => {
    const ids = parseAgentMentions('@Writer and again @Writer', AGENTS).map((a) => a.id)
    expect(ids).toEqual(['writerAgent'])
  })

  it('matches a name containing spaces', () => {
    expect(parseAgentMentions('ask @Software Architect', AGENTS).map((a) => a.id)).toEqual([
      'architectAgent'
    ])
  })

  it('prefers the longest matching name', () => {
    // "@Review Helper" must not resolve to "Reviewer" or leave a partial match
    expect(parseAgentMentions('ask @Review Helper', AGENTS).map((a) => a.id)).toEqual([
      'reviewHelperAgent'
    ])
  })

  it('does not double-count a name that prefixes a longer one', () => {
    // "@Writer Pro" must resolve to "Writer Pro" only, not also "Writer"
    expect(parseAgentMentions('ask @Writer Pro to draft', AGENTS).map((a) => a.id)).toEqual([
      'writerProAgent'
    ])
  })

  it('still matches the shorter name on its own', () => {
    expect(parseAgentMentions('ask @Writer to draft', AGENTS).map((a) => a.id)).toEqual([
      'writerAgent'
    ])
  })

  it('matches both when each is mentioned separately', () => {
    const ids = parseAgentMentions('@Writer Pro drafts, @Writer edits', AGENTS).map((a) => a.id)
    expect(ids).toEqual(['writerProAgent', 'writerAgent'])
  })

  it('does not match a longer name from a shorter mention', () => {
    // There is no agent literally named "Review"
    expect(parseAgentMentions('ask @Review to help', AGENTS)).toEqual([])
  })

  it('is case-insensitive', () => {
    expect(parseAgentMentions('ask @reviewer', AGENTS).map((a) => a.id)).toEqual(['reviewerAgent'])
  })

  it('ignores an unknown mention', () => {
    expect(parseAgentMentions('ask @Nobody', AGENTS)).toEqual([])
  })

  it('ignores an email address', () => {
    expect(parseAgentMentions('mail me at foo@Writer.com', AGENTS)).toEqual([])
  })

  it('excludes the mentioning agent itself', () => {
    expect(parseAgentMentions('@Caller do it', AGENTS, 'callerAgent')).toEqual([])
  })

  it('excludes special page agents', () => {
    expect(parseAgentMentions('@React Generator go', AGENTS)).toEqual([])
  })
})

describe('applyDelegationAllowlist', () => {
  it('returns the same array when invokeAgent is not enabled', () => {
    const tools = [readFilesTool()]
    expect(applyDelegationAllowlist(tools, [AGENTS[0]])).toBe(tools)
  })

  it('drops invokeAgent when nothing was mentioned', () => {
    const tools = [readFilesTool(), invokeAgentTool()]
    const result = applyDelegationAllowlist(tools, [])

    expect(result.map((t) => t.toolSpec?.name)).toEqual(['readFiles'])
  })

  it('constrains agentId to the mentioned agents', () => {
    const tools = [readFilesTool(), invokeAgentTool()]
    const result = applyDelegationAllowlist(tools, [AGENTS[0], AGENTS[2]])
    const spec = result.find((t) => t.toolSpec?.name === 'invokeAgent')?.toolSpec

    expect((spec?.inputSchema?.json as any).properties.agentId.enum).toEqual([
      'reviewerAgent',
      'writerAgent'
    ])
    expect(spec?.description).toContain('Reviewer')
  })

  it('does not mutate the input tools', () => {
    const tools = [readFilesTool(), invokeAgentTool()]
    const before = JSON.stringify(tools)

    applyDelegationAllowlist(tools, [AGENTS[0]])

    expect(JSON.stringify(tools)).toBe(before)
  })

  it('preserves the enabled flag', () => {
    const tools = [invokeAgentTool()]
    const result = applyDelegationAllowlist(tools, [AGENTS[0]])
    expect(result[0].enabled).toBe(true)
  })
})

describe('hasInvokeAgentTool', () => {
  it('detects the tool', () => {
    expect(hasInvokeAgentTool([readFilesTool(), invokeAgentTool()])).toBe(true)
    expect(hasInvokeAgentTool([readFilesTool()])).toBe(false)
  })
})

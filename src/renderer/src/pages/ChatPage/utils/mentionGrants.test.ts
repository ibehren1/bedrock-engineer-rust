import type { CustomAgent, LocalKnowledgeBaseRef } from '@/types/agent-chat'
import {
  applyPromptToGrants,
  deriveGrants,
  EMPTY_GRANTS,
  resolveGrants,
  setGrant
} from './mentionGrants'

const agent = (id: string, name: string, kbs: LocalKnowledgeBaseRef[] = []): CustomAgent =>
  ({
    id,
    name,
    description: '',
    system: '',
    scenarios: [],
    localKnowledgeBases: kbs
  }) as CustomAgent

const AGENTS: CustomAgent[] = [
  agent('me', 'Me', [{ id: 'kb1', name: 'Manuals' }]),
  agent('other', 'Other', [{ id: 'kb3', name: 'Recipes' }]),
  agent('writer', 'Writer'),
  agent('reviewer', 'Reviewer')
]

const derive = (prompts: string[]) => deriveGrants(prompts, AGENTS, 'me')

describe('deriveGrants', () => {
  it('keeps agents and knowledge bases mentioned in earlier prompts', () => {
    expect(derive(['ask @Writer, use @Recipes', 'now fix the typo'])).toEqual({
      agentIds: ['writer'],
      knowledgeBases: [{ agentId: 'other', kbId: 'kb3' }]
    })
  })

  it('collects in the order things were first turned on', () => {
    expect(derive(['@Writer draft it', 'then @Reviewer', '@Writer again']).agentIds).toEqual([
      'writer',
      'reviewer'
    ])
  })

  it('turns things off with "disable @Name" and back on with a new mention', () => {
    expect(derive(['@Writer @Manuals', 'Disable @Writer and disable @Manuals'])).toEqual(
      EMPTY_GRANTS
    )
    expect(derive(['@Writer', 'disable @Writer', 'ok @Writer go']).agentIds).toEqual(['writer'])
  })

  it('only disables the mention right after "disable"', () => {
    expect(derive(['@Writer and @Reviewer', 'disable @Reviewer, keep @Writer']).agentIds).toEqual([
      'writer'
    ])
  })

  it('does not treat words ending in "disable" as the keyword', () => {
    expect(derive(['undisable @Writer']).agentIds).toEqual(['writer'])
  })

  it('never grants the chat’s own agent', () => {
    expect(derive(['@Me do it']).agentIds).toEqual([])
  })
})

describe('applyPromptToGrants', () => {
  it('reports what the prompt disabled', () => {
    const start = derive(['@Writer @Recipes'])
    const { grants, disabled } = applyPromptToGrants(start, 'disable @Recipes please', AGENTS, 'me')
    expect(grants.knowledgeBases).toEqual([])
    expect(disabled.map((d) => (d.kind === 'kb' ? d.kb.kbId : d.agent.id))).toEqual(['kb3'])
  })

  it('reports nothing when a later mention in the same prompt turns it back on', () => {
    const { disabled } = applyPromptToGrants(
      EMPTY_GRANTS,
      'disable @Writer, no wait, @Writer',
      AGENTS,
      'me'
    )
    expect(disabled).toEqual([])
  })
})

describe('setGrant', () => {
  it('turns an agent or knowledge base on and off', () => {
    const on = setGrant(EMPTY_GRANTS, { kind: 'kb', agentId: 'me', kbId: 'kb1' }, true)
    expect(on.knowledgeBases).toEqual([{ agentId: 'me', kbId: 'kb1' }])
    expect(setGrant(on, { kind: 'kb', agentId: 'me', kbId: 'kb1' }, false)).toEqual(EMPTY_GRANTS)
    expect(setGrant(EMPTY_GRANTS, { kind: 'agent', agentId: 'writer' }, false)).toBe(EMPTY_GRANTS)
  })
})

describe('resolveGrants', () => {
  it('drops agents and knowledge bases that no longer exist', () => {
    const resolved = resolveGrants(
      {
        agentIds: ['gone', 'writer'],
        knowledgeBases: [
          { agentId: 'other', kbId: 'kb3' },
          { agentId: 'other', kbId: 'deleted' }
        ]
      },
      AGENTS,
      'me'
    )
    expect(resolved.agents.map((a) => a.id)).toEqual(['writer'])
    expect(resolved.knowledgeBases.map((kb) => kb.kbId)).toEqual(['kb3'])
  })
})

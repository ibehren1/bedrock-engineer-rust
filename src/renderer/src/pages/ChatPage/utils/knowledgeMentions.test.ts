import type { CustomAgent, LocalKnowledgeBaseRef, ToolState } from '@/types/agent-chat'
import {
  buildMentionedKnowledgeBasesPrompt,
  getMentionableKnowledgeBases,
  isMentionedKnowledgeBaseTool,
  listMentionedKnowledgeBases,
  parseMentions,
  toGrants,
  withMentionedKnowledgeBaseTools
} from './knowledgeMentions'

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
  agent('me', 'Me', [
    { id: 'kb1', name: 'Manuals', description: 'Product manuals' },
    { id: 'kb2', name: 'Notes' }
  ]),
  agent('other', 'Other', [
    { id: 'kb3', name: 'Notes' },
    { id: 'kb4', name: 'Recipes' },
    { id: 'kb5', name: 'Writer' }
  ]),
  agent('writer', 'Writer')
]

describe('getMentionableKnowledgeBases', () => {
  it("lists every agent's knowledge bases, own ones first", () => {
    const kbs = getMentionableKnowledgeBases(AGENTS, 'me')
    expect(kbs.map((kb) => kb.kbId)).toEqual(['kb1', 'kb2', 'kb3', 'kb4', 'kb5'])
    expect(kbs.filter((kb) => kb.isOwn).map((kb) => kb.kbId)).toEqual(['kb1', 'kb2'])
  })

  it('qualifies a name another knowledge base or an agent has', () => {
    const mention = Object.fromEntries(
      getMentionableKnowledgeBases(AGENTS, 'me').map((kb) => [kb.kbId, kb.mention])
    )
    expect(mention).toEqual({
      kb1: 'Manuals',
      // The own one keeps the bare name.
      kb2: 'Notes',
      kb3: 'Other/Notes',
      kb4: 'Recipes',
      // An agent is called Writer.
      kb5: 'Other/Writer'
    })
  })

  it('qualifies both when neither is the current agent’s', () => {
    const mention = getMentionableKnowledgeBases(AGENTS, 'writer')
      .filter((kb) => kb.kbName === 'Notes')
      .map((kb) => kb.mention)
    expect(mention).toEqual(['Me/Notes', 'Other/Notes'])
  })
})

describe('parseMentions', () => {
  it('finds agents and knowledge bases in one pass', () => {
    const { agents, knowledgeBases } = parseMentions(
      'Ask @Writer about @Other/Notes and @Manuals, then @notes.',
      AGENTS,
      'me'
    )
    expect(agents.map((a) => a.id)).toEqual(['writer'])
    expect(knowledgeBases.map((kb) => kb.kbId)).toEqual(['kb3', 'kb1', 'kb2'])
  })

  it('does not read a qualified knowledge base as its agent', () => {
    const { agents, knowledgeBases } = parseMentions('see @Other/Writer', AGENTS, 'me')
    expect(agents).toEqual([])
    expect(knowledgeBases.map((kb) => kb.kbId)).toEqual(['kb5'])
  })

  it('ignores email addresses and unknown names', () => {
    expect(parseMentions('mail me@Manuals.com or @Nothing', AGENTS, 'me').knowledgeBases).toEqual(
      []
    )
  })
})

describe('turn helpers', () => {
  const kbs = parseMentions('@Manuals @Recipes', AGENTS, 'me').knowledgeBases

  it('adds the knowledge base tools, limited to the mentioned names', () => {
    const other: ToolState = { enabled: true, toolSpec: { name: 'think' } } as ToolState
    const tools = withMentionedKnowledgeBaseTools([other], kbs)
    expect(tools.map((t) => t.toolSpec?.name)).toEqual([
      'think',
      'searchKnowledgeBase',
      'listKnowledgeBases',
      'listKnowledgeBaseSources'
    ])
    const schema = tools[1].toolSpec?.inputSchema?.json as {
      properties: { knowledgeBase: { enum: string[] } }
    }
    expect(schema.properties.knowledgeBase.enum).toEqual(['Manuals', 'Recipes'])
    expect(withMentionedKnowledgeBaseTools([other], [])).toEqual([other])
  })

  it('builds grants, the prompt block and the list', () => {
    expect(toGrants(kbs)).toEqual([
      { agentId: 'me', kbId: 'kb1' },
      { agentId: 'other', kbId: 'kb4' }
    ])
    const prompt = buildMentionedKnowledgeBasesPrompt(kbs)
    expect(prompt).toContain('- Manuals: Product manuals')
    expect(prompt).toContain('- Recipes (from the agent "Other")')
    expect(prompt).toContain('"Cite as" link right after each sentence')
    expect(buildMentionedKnowledgeBasesPrompt([])).toBe('')
    expect(listMentionedKnowledgeBases(kbs)).toBe(
      '- Manuals (id: kb1). Product manuals\n- Recipes (id: kb4) (agent "Other")'
    )
    expect(isMentionedKnowledgeBaseTool('searchKnowledgeBase')).toBe(true)
    expect(isMentionedKnowledgeBaseTool('think')).toBe(false)
  })
})

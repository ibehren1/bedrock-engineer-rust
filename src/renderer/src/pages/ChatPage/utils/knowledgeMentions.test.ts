import type { CustomAgent, LocalKnowledgeBaseRef, ToolState } from '@/types/agent-chat'
import {
  buildMentionedKnowledgeBasesPrompt,
  getMentionableKnowledgeBases,
  isMentionedKnowledgeBaseTool,
  listMentionedKnowledgeBases,
  parseMentions,
  toGrants,
  withMentionedKnowledgeBaseTools,
  type KnowledgeBaseListItem
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

const KBS: KnowledgeBaseListItem[] = [
  { id: 'kb1', name: 'Manuals', description: 'Product manuals' },
  { id: 'kb2', name: 'Notes' },
  { id: 'kb4', name: 'Recipes' },
  { id: 'kb5', name: 'Writer' },
  // A duplicate name from before names were unique.
  { id: 'kb6', name: 'notes' }
]

const AGENTS: CustomAgent[] = [
  agent('me', 'Me', [{ id: 'kb4', name: 'Recipes' }]),
  agent('other', 'Other', [{ id: 'kb2', name: 'Notes' }]),
  agent('writer', 'Writer')
]

describe('getMentionableKnowledgeBases', () => {
  it('lists every knowledge base, the attached ones first', () => {
    const kbs = getMentionableKnowledgeBases(KBS, AGENTS, 'me')
    expect(kbs.map((kb) => kb.kbId)).toEqual(['kb4', 'kb1', 'kb2', 'kb5'])
    expect(kbs.filter((kb) => kb.attached).map((kb) => kb.kbId)).toEqual(['kb4'])
  })

  it('prefixes a name an agent has with kb/', () => {
    const mention = Object.fromEntries(
      getMentionableKnowledgeBases(KBS, AGENTS, 'me').map((kb) => [kb.kbId, kb.mention])
    )
    expect(mention).toEqual({ kb1: 'Manuals', kb2: 'Notes', kb4: 'Recipes', kb5: 'kb/Writer' })
  })

  it('uses the bare name when the clashing agent is the current one', () => {
    const writer = getMentionableKnowledgeBases(KBS, AGENTS, 'writer').find(
      (kb) => kb.kbId === 'kb5'
    )
    expect(writer?.mention).toBe('Writer')
  })
})

describe('parseMentions', () => {
  it('finds agents and knowledge bases in one pass', () => {
    const { agents, knowledgeBases } = parseMentions(
      'Ask @Writer about @Notes and @Manuals, then @notes.',
      AGENTS,
      KBS,
      'me'
    )
    expect(agents.map((a) => a.id)).toEqual(['writer'])
    expect(knowledgeBases.map((kb) => kb.kbId)).toEqual(['kb2', 'kb1'])
  })

  it('does not read a prefixed knowledge base as the agent', () => {
    const { agents, knowledgeBases } = parseMentions('see @kb/Writer', AGENTS, KBS, 'me')
    expect(agents).toEqual([])
    expect(knowledgeBases.map((kb) => kb.kbId)).toEqual(['kb5'])
  })

  it('ignores email addresses and unknown names', () => {
    expect(
      parseMentions('mail me@Manuals.com or @Nothing', AGENTS, KBS, 'me').knowledgeBases
    ).toEqual([])
  })
})

describe('turn helpers', () => {
  const kbs = parseMentions('@Manuals @Recipes', AGENTS, KBS, 'me').knowledgeBases

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
    expect(toGrants(kbs)).toEqual([{ kbId: 'kb1' }, { kbId: 'kb4' }])
    const prompt = buildMentionedKnowledgeBasesPrompt(kbs)
    expect(prompt).toContain('- Manuals: Product manuals')
    expect(prompt).toContain('- Recipes\n')
    expect(prompt).toContain('"Cite as" link right after each sentence')
    expect(buildMentionedKnowledgeBasesPrompt([])).toBe('')
    expect(listMentionedKnowledgeBases(kbs)).toBe(
      '- Manuals (id: kb1). Product manuals\n- Recipes (id: kb4)'
    )
    expect(isMentionedKnowledgeBaseTool('searchKnowledgeBase')).toBe(true)
    expect(isMentionedKnowledgeBaseTool('think')).toBe(false)
  })
})

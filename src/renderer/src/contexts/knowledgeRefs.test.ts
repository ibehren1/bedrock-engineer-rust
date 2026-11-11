import type { CustomAgent, LocalKnowledgeBaseRef } from '@/types/agent-chat'
import { agentsUsing, reconcileKnowledgeRefs, setKnowledgeRef } from './knowledgeRefs'

const agent = (id: string, kbs?: LocalKnowledgeBaseRef[]): CustomAgent =>
  ({
    id,
    name: id,
    description: '',
    system: '',
    scenarios: [],
    localKnowledgeBases: kbs
  }) as CustomAgent

const KBS = [
  { id: 'kb1', name: 'Manuals', description: 'Product manuals' },
  { id: 'kb2', name: 'Notes', description: '' }
]

describe('reconcileKnowledgeRefs', () => {
  it('returns null when every ref is current', () => {
    const agents = [
      agent('a', [{ id: 'kb1', name: 'Manuals', description: 'Product manuals' }]),
      agent('b'),
      agent('c', [])
    ]
    expect(reconcileKnowledgeRefs(agents, KBS)).toBeNull()
  })

  it('treats a missing description like an empty one', () => {
    expect(reconcileKnowledgeRefs([agent('a', [{ id: 'kb2', name: 'Notes' }])], KBS)).toBeNull()
  })

  it('drops refs to deleted knowledge bases and refreshes names and descriptions', () => {
    const agents = [
      agent('a', [
        { id: 'gone', name: 'Old' },
        { id: 'kb1', name: 'Old name' }
      ]),
      agent('b', [{ id: 'kb2', name: 'Notes', description: '' }])
    ]
    const next = reconcileKnowledgeRefs(agents, KBS)
    expect(next?.[0].localKnowledgeBases).toEqual([
      { id: 'kb1', name: 'Manuals', description: 'Product manuals' }
    ])
    // Unchanged agents keep their object.
    expect(next?.[1]).toBe(agents[1])
  })

  it('drops duplicate refs', () => {
    const next = reconcileKnowledgeRefs(
      [
        agent('a', [
          { id: 'kb2', name: 'Notes' },
          { id: 'kb2', name: 'Notes' }
        ])
      ],
      KBS
    )
    expect(next?.[0].localKnowledgeBases?.map((r) => r.id)).toEqual(['kb2'])
  })

  it('detaches everything when the list is empty', () => {
    const next = reconcileKnowledgeRefs([agent('a', [{ id: 'kb1', name: 'Manuals' }])], [])
    expect(next?.[0].localKnowledgeBases).toEqual([])
  })
})

describe('setKnowledgeRef', () => {
  it('attaches and detaches', () => {
    const on = setKnowledgeRef(undefined, KBS[0], true)
    expect(on).toEqual([{ id: 'kb1', name: 'Manuals', description: 'Product manuals' }])
    expect(setKnowledgeRef(on, KBS[0], true)).toBe(on)
    expect(setKnowledgeRef(on, KBS[0], false)).toEqual([])
  })
})

describe('agentsUsing', () => {
  it('lists the agents that attach a knowledge base', () => {
    const agents = [agent('a', [{ id: 'kb1', name: 'Manuals' }]), agent('b'), agent('c', [])]
    expect(agentsUsing(agents, 'kb1').map((a) => a.id)).toEqual(['a'])
  })
})

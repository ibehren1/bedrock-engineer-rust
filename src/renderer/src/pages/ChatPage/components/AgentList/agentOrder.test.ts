import { reorderAgentIds, sortAgentsByOrder } from './agentOrder'

describe('reorderAgentIds', () => {
  const ids = ['a', 'b', 'c', 'd']

  it('moves an agent downwards to after the target', () => {
    expect(reorderAgentIds(ids, 'a', 'c')).toEqual(['b', 'c', 'a', 'd'])
  })

  it('moves an agent upwards to before the target', () => {
    expect(reorderAgentIds(ids, 'd', 'b')).toEqual(['a', 'd', 'b', 'c'])
  })

  it('moves an agent to the end of the list', () => {
    expect(reorderAgentIds(ids, 'a', 'd')).toEqual(['b', 'c', 'd', 'a'])
  })

  it('keeps the list unchanged when dropped on itself or on an unknown id', () => {
    expect(reorderAgentIds(ids, 'b', 'b')).toEqual(ids)
    expect(reorderAgentIds(ids, 'b', 'zzz')).toEqual(ids)
    expect(reorderAgentIds(ids, 'zzz', 'b')).toEqual(ids)
  })
})

describe('sortAgentsByOrder', () => {
  const agents = [{ id: 'a' }, { id: 'b' }, { id: 'c' }]

  it('returns the agents untouched when no order is stored', () => {
    expect(sortAgentsByOrder(agents, [])).toEqual(agents)
  })

  it('applies the stored order', () => {
    expect(sortAgentsByOrder(agents, ['c', 'a', 'b'])).toEqual([
      { id: 'c' },
      { id: 'a' },
      { id: 'b' }
    ])
  })

  it('puts agents missing from the stored order last, in their original order', () => {
    expect(sortAgentsByOrder([{ id: 'a' }, { id: 'new' }, { id: 'c' }], ['c', 'a'])).toEqual([
      { id: 'c' },
      { id: 'a' },
      { id: 'new' }
    ])
  })

  it('ignores stored ids that no longer exist', () => {
    expect(sortAgentsByOrder(agents, ['gone', 'b', 'a', 'c'])).toEqual([
      { id: 'b' },
      { id: 'a' },
      { id: 'c' }
    ])
  })
})

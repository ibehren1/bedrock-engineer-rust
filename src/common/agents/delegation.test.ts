import {
  buildInvokeAgentToolSpec,
  canDelegate,
  filterDelegationTargets,
  MAX_DELEGATION_DEPTH
} from './delegation'

const baseSpec = {
  name: 'invokeAgent',
  description: 'Delegate a task.',
  inputSchema: {
    json: {
      type: 'object',
      properties: {
        agentId: { type: 'string', description: 'target' },
        task: { type: 'string' }
      },
      required: ['agentId', 'task']
    }
  }
} as any

describe('filterDelegationTargets', () => {
  it('removes agents already in the caller chain', () => {
    expect(filterDelegationTargets(['a', 'b', 'c'], ['a'], 'b')).toEqual(['c'])
  })

  it('removes the caller itself', () => {
    expect(filterDelegationTargets(['a', 'b'], [], 'a')).toEqual(['b'])
  })

  it('dedupes and drops empty ids', () => {
    expect(filterDelegationTargets(['a', 'a', '', 'b'], [], undefined)).toEqual(['a', 'b'])
  })

  it('returns an empty list when everything is excluded', () => {
    expect(filterDelegationTargets(['a'], ['a'], 'a')).toEqual([])
  })
})

describe('canDelegate', () => {
  it('permits delegation below the depth limit with targets available', () => {
    expect(canDelegate(0, 1)).toBe(true)
    expect(canDelegate(MAX_DELEGATION_DEPTH - 1, 1)).toBe(true)
  })

  it('refuses at or beyond the depth limit', () => {
    expect(canDelegate(MAX_DELEGATION_DEPTH, 5)).toBe(false)
    expect(canDelegate(MAX_DELEGATION_DEPTH + 1, 5)).toBe(false)
  })

  it('refuses when no targets remain', () => {
    expect(canDelegate(0, 0)).toBe(false)
  })
})

describe('buildInvokeAgentToolSpec', () => {
  it('returns null when nothing may be delegated to', () => {
    expect(buildInvokeAgentToolSpec(baseSpec, [])).toBeNull()
  })

  it('returns null for a missing base spec', () => {
    expect(buildInvokeAgentToolSpec(undefined, [{ id: 'a', name: 'A' }])).toBeNull()
  })

  it('constrains agentId to the permitted ids', () => {
    const spec = buildInvokeAgentToolSpec(baseSpec, [
      { id: 'reviewer', name: 'Reviewer' },
      { id: 'writer', name: 'Writer' }
    ])

    expect((spec?.inputSchema?.json as any).properties.agentId.enum).toEqual(['reviewer', 'writer'])
  })

  it('lists every id and name in the description', () => {
    const spec = buildInvokeAgentToolSpec(baseSpec, [
      { id: 'reviewer', name: 'Reviewer', description: 'Reviews code' }
    ])

    expect(spec?.description).toContain('reviewer')
    expect(spec?.description).toContain('Reviewer')
    expect(spec?.description).toContain('Reviews code')
    // The original description must survive
    expect(spec?.description).toContain('Delegate a task.')
  })

  it('truncates a very long target description', () => {
    const spec = buildInvokeAgentToolSpec(baseSpec, [
      { id: 'a', name: 'A', description: 'x'.repeat(500) }
    ])

    expect(spec?.description).toContain('…')
    expect(spec?.description).not.toContain('x'.repeat(400))
  })

  it('does not mutate the base spec', () => {
    const before = JSON.stringify(baseSpec)
    buildInvokeAgentToolSpec(baseSpec, [{ id: 'a', name: 'A' }])
    expect(JSON.stringify(baseSpec)).toBe(before)
  })
})

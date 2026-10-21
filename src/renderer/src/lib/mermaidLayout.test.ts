import { flipDirection, needsReorient, prefersFlipped } from './mermaidLayout'

describe('flipDirection', () => {
  it('turns left-right flowcharts top-bottom', () => {
    expect(flipDirection('flowchart LR\n  A --> B')).toBe('flowchart TB\n  A --> B')
    expect(flipDirection('graph RL\n  A --> B')).toBe('graph TB\n  A --> B')
  })

  it('turns top-bottom flowcharts left-right', () => {
    expect(flipDirection('flowchart TD\n  A --> B')).toBe('flowchart LR\n  A --> B')
    expect(flipDirection('graph BT\nA-->B')).toBe('graph LR\nA-->B')
  })

  it('looks past leading comments and blank lines', () => {
    expect(flipDirection('%% network\n\nflowchart LR\nA-->B')).toBe(
      '%% network\n\nflowchart TB\nA-->B'
    )
  })

  it('changes only the top-level direction, not subgraph ones', () => {
    const code = 'flowchart LR\n  subgraph S\n    direction LR\n    A --> B\n  end'
    expect(flipDirection(code)).toBe(
      'flowchart TB\n  subgraph S\n    direction LR\n    A --> B\n  end'
    )
  })

  it('leaves other diagram types and direction-less flowcharts alone', () => {
    expect(flipDirection('sequenceDiagram\n  A->>B: hi')).toBeNull()
    expect(flipDirection('flowchart\n  A --> B')).toBeNull()
    expect(flipDirection('flowchart LRX\n  A --> B')).toBeNull()
  })
})

describe('needsReorient', () => {
  it('only for lopsided shapes', () => {
    expect(needsReorient(5.9)).toBe(true)
    expect(needsReorient(0.3)).toBe(true)
    expect(needsReorient(1.5)).toBe(false)
    expect(needsReorient(null)).toBe(false)
  })
})

describe('prefersFlipped', () => {
  it('picks the shape closer to the target ratio', () => {
    expect(prefersFlipped(5.94, 0.96)).toBe(true)
    expect(prefersFlipped(2.6, 0.2)).toBe(false)
  })
})

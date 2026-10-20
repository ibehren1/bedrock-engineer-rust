import { nextAttention, type AttentionSnapshot } from './panelAttention'

const snap = (over: Partial<AttentionSnapshot> = {}): AttentionSnapshot => ({
  sessionId: 's1',
  visible: true,
  hasSandbox: false,
  touchedCount: 0,
  ...over
})

describe('nextAttention', () => {
  it('pulses when the strip first appears in a chat, with a dot only if there is something to see', () => {
    expect(nextAttention(undefined, snap(), false, false)).toEqual({ pulse: true, badge: false })
    expect(nextAttention(undefined, snap({ touchedCount: 2 }), false, false)).toEqual({
      pulse: true,
      badge: true
    })
    // Switching chats starts over.
    expect(
      nextAttention(snap({ touchedCount: 3 }), snap({ sessionId: 's2' }), false, true)
    ).toEqual({ pulse: true, badge: false })
    // The strip appearing (a project folder was chosen).
    expect(nextAttention(snap({ visible: false }), snap(), false, false).pulse).toBe(true)
  })

  it('pulses and shows the dot when a sandbox arrives or new files are touched', () => {
    expect(nextAttention(snap(), snap({ hasSandbox: true }), false, false)).toEqual({
      pulse: true,
      badge: true
    })
    expect(
      nextAttention(snap({ touchedCount: 1 }), snap({ touchedCount: 2 }), false, false)
    ).toEqual({ pulse: true, badge: true })
  })

  it('keeps the dot without pulsing when nothing new happens', () => {
    expect(nextAttention(snap(), snap(), false, true)).toEqual({ pulse: false, badge: true })
    expect(nextAttention(snap(), snap(), false, false)).toEqual({ pulse: false, badge: false })
    // A sandbox going away is not news.
    expect(nextAttention(snap({ hasSandbox: true }), snap(), false, true)).toEqual({
      pulse: false,
      badge: true
    })
  })

  it('never draws attention while the panel is open or the strip is hidden', () => {
    expect(nextAttention(snap(), snap({ touchedCount: 5 }), true, true)).toEqual({
      pulse: false,
      badge: false
    })
    expect(nextAttention(undefined, snap(), true, false).pulse).toBe(false)
    expect(nextAttention(snap(), snap({ visible: false, hasSandbox: true }), false, true)).toEqual({
      pulse: false,
      badge: false
    })
  })

  it('does not pulse when the panel is closed again', () => {
    // Files touched while it was open were already seen.
    const seen = snap({ touchedCount: 4 })
    expect(nextAttention(seen, seen, false, false)).toEqual({ pulse: false, badge: false })
  })
})

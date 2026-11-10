import { streamScrollKeys } from './useStreamingAutoScroll'

describe('streamScrollKeys', () => {
  test('blockKey stays put while the last message grows; signature changes', () => {
    const a = streamScrollKeys(2, { content: [{ text: 'Hel' }] })
    const b = streamScrollKeys(2, { content: [{ text: 'Hello' }] })
    expect(b.blockKey).toBe(a.blockKey)
    expect(b.signature).not.toBe(a.signature)
  })

  test('thinking text growth changes the signature', () => {
    const msg = (t: string) => ({
      content: [{ reasoningContent: { reasoningText: { text: t } } }, { text: '' }]
    })
    const a = streamScrollKeys(2, msg('a'))
    const b = streamScrollKeys(2, msg('abc'))
    expect(b.blockKey).toBe(a.blockKey)
    expect(b.signature).not.toBe(a.signature)
  })

  test('streamed tool input growth changes the signature', () => {
    const msg = (input: string) => ({ content: [{ text: 'ok' }, { toolUse: { input } }] })
    const a = streamScrollKeys(2, msg('{"pa'))
    const b = streamScrollKeys(2, msg('{"path": "x"'))
    expect(b.blockKey).toBe(a.blockKey)
    expect(b.signature).not.toBe(a.signature)
  })

  test('parsed tool input does not throw', () => {
    expect(() =>
      streamScrollKeys(1, { content: [{ toolUse: { input: { path: 'x' } } }] })
    ).not.toThrow()
  })

  test('new block or new message changes blockKey', () => {
    const base = streamScrollKeys(2, { content: [{ text: 'hi' }] })
    expect(streamScrollKeys(2, { content: [{ text: 'hi' }, { toolUse: {} }] }).blockKey).not.toBe(
      base.blockKey
    )
    expect(streamScrollKeys(3, { content: [{ text: 'hi' }] }).blockKey).not.toBe(base.blockKey)
  })

  test('handles an empty chat', () => {
    expect(streamScrollKeys(0, undefined)).toEqual({ blockKey: '0:0', signature: '0:0:0' })
  })
})

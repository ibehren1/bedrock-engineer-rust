import {
  HTML_PREVIEW_READY,
  HTML_PREVIEW_RENDER,
  HTML_PREVIEW_SANDBOX,
  previewReply
} from './htmlPreview'

describe('html preview sandbox', () => {
  test('the iframe never gets same-origin access to the app', () => {
    expect(HTML_PREVIEW_SANDBOX.split(/\s+/)).toEqual(['allow-scripts'])
  })

  test('answers its own frame once it is ready', () => {
    const frame = {}
    expect(
      previewReply({ source: frame, data: { type: HTML_PREVIEW_READY } }, frame, '<p>hi</p>')
    ).toEqual({ type: HTML_PREVIEW_RENDER, html: '<p>hi</p>' })
  })

  test('ignores other frames and other messages', () => {
    const frame = {}
    const ready = { type: HTML_PREVIEW_READY }
    expect(previewReply({ source: {}, data: ready }, frame, 'x')).toBeNull()
    expect(previewReply({ source: null, data: ready }, null, 'x')).toBeNull()
    expect(previewReply({ source: frame, data: { type: 'other' } }, frame, 'x')).toBeNull()
    expect(previewReply({ source: frame, data: 'html-preview:ready' }, frame, 'x')).toBeNull()
    expect(previewReply({ source: frame, data: null }, frame, 'x')).toBeNull()
  })
})

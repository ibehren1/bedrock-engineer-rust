import { describe, expect, it } from '@jest/globals'
import { rehypeSoftBreaks } from './softBreaks'

const el = (tagName: string, ...children: object[]) => ({ type: 'element', tagName, children })
const text = (value: string) => ({ type: 'text', value })

describe('rehypeSoftBreaks', () => {
  it('turns soft breaks in prose into spaces and leaves code alone', () => {
    const tree: any = {
      type: 'root',
      children: [
        el('p', text('again in the\n'), el('a', text('Glossary')), text(' at the end and  \n  ')),
        el('pre', el('code', text('line 1\nline 2\n')))
      ]
    }
    rehypeSoftBreaks()(tree)
    expect(tree.children[0].children[0].value).toBe('again in the ')
    expect(tree.children[0].children[2].value).toBe(' at the end and ')
    expect(tree.children[1].children[0].children[0].value).toBe('line 1\nline 2\n')
  })
})

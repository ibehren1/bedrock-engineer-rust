import { ContentBlock, Message } from '@aws-sdk/client-bedrock-runtime'
import { injectAttachmentBlocks } from './attachmentContext'

const blocks: ContentBlock[] = [
  { text: '<chat_attachments>…</chat_attachments>' } as ContentBlock,
  { image: { format: 'png', source: { bytes: 'AAAA' } } } as unknown as ContentBlock
]

const conversation = (): Message[] => [
  { role: 'user', content: [{ text: 'first question' }] },
  { role: 'assistant', content: [{ text: 'first answer' }] },
  { role: 'user', content: [{ text: 'second question' }] }
]

it('puts the blocks in front of the newest user prompt', () => {
  const result = injectAttachmentBlocks(conversation(), blocks)

  expect(result[2].content).toEqual([...blocks, { text: 'second question' }])
  // Earlier turns stay bare, so a document is never sent once per turn.
  expect(result[0].content).toEqual([{ text: 'first question' }])
})

it('never changes the message count, which cache points are indexed by', () => {
  const messages = conversation()
  expect(injectAttachmentBlocks(messages, blocks)).toHaveLength(messages.length)
})

it('leaves the input array and its messages untouched', () => {
  const messages = conversation()
  const snapshot = JSON.parse(JSON.stringify(messages))

  injectAttachmentBlocks(messages, blocks)

  expect(messages).toEqual(snapshot)
})

it('returns the same array when there is nothing to inject', () => {
  const messages = conversation()
  expect(injectAttachmentBlocks(messages, [])).toBe(messages)
})

it('is idempotent across the requests of one turn', () => {
  const messages = conversation()
  expect(injectAttachmentBlocks(messages, blocks)).toEqual(injectAttachmentBlocks(messages, blocks))
})

it('skips a tool-result message and uses the prompt behind it', () => {
  // Converse rejects a user message that mixes toolResult with text or image blocks.
  const messages: Message[] = [
    { role: 'user', content: [{ text: 'run the tool' }] },
    {
      role: 'assistant',
      content: [{ toolUse: { toolUseId: '1', name: 'readFiles', input: {} } }]
    },
    {
      role: 'user',
      content: [{ toolResult: { toolUseId: '1', content: [{ text: 'done' }], status: 'success' } }]
    }
  ]

  const result = injectAttachmentBlocks(messages, blocks)

  expect(result[0].content).toEqual([...blocks, { text: 'run the tool' }])
  expect(result[2].content).toEqual(messages[2].content)
})

it('sends without attachments rather than guessing when no prompt survives trimming', () => {
  const warn = jest.spyOn(console, 'warn').mockImplementation(() => undefined)
  const messages: Message[] = [{ role: 'assistant', content: [{ text: 'only an answer' }] }]

  expect(injectAttachmentBlocks(messages, blocks)).toBe(messages)
  expect(warn).toHaveBeenCalled()

  warn.mockRestore()
})

import { ContentBlock, Message } from '@aws-sdk/client-bedrock-runtime'

/**
 * Put this turn's attachment blocks in front of the newest real user prompt.
 *
 * Returns a new array and leaves `messages` — and every message object in it — untouched.
 * That is what keeps the blocks out of chat history: only the copy handed to Converse carries
 * them, while the array `persistMessage` writes and the message list renders stays bare. It is
 * also why the tool-use recursion cannot duplicate them, since every request re-derives its
 * payload from the same pristine input.
 *
 * The anchor is the last `user` message carrying a text / guardContent / image block and no
 * toolUse or toolResult block: Converse rejects a message that mixes toolResult with other
 * content, so tool-result messages are never touched.
 *
 * The message count never changes. `PromptCacheManager` places cache points by index and the
 * hook remembers the last one across requests, so adding or dropping a message here would move
 * them; only the anchor's content array grows.
 */
export const injectAttachmentBlocks = (messages: Message[], blocks: ContentBlock[]): Message[] => {
  if (blocks.length === 0) return messages

  const anchor = messages.reduceRight((found: number, message, index) => {
    if (found >= 0 || message.role !== 'user') return found

    const content = message.content ?? []
    const isPrompt = content.some(
      (block) => 'text' in block || 'guardContent' in block || 'image' in block
    )
    const isToolTraffic = content.some((block) => 'toolResult' in block || 'toolUse' in block)

    return isPrompt && !isToolTraffic ? index : found
  }, -1)

  if (anchor < 0) {
    // Only reachable when context trimming has pushed the prompt out of the window, by which
    // point earlier requests in the turn already carried the attachments.
    console.warn('No user prompt to attach files to; sending without attachment context')
    return messages
  }

  const updated = [...messages]
  updated[anchor] = {
    ...updated[anchor],
    content: [...blocks, ...(updated[anchor].content ?? [])]
  }
  return updated
}

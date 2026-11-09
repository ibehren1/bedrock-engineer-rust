import type { Message } from '@aws-sdk/client-bedrock-runtime'

/** The text of the user's earlier prompts in a conversation (not tool results), oldest first. */
export function previousUserPrompts(messages: Message[]): string[] {
  return messages
    .filter((m) => m.role === 'user')
    .map((m) =>
      (m.content ?? [])
        .map(
          (c) =>
            ('text' in c && c.text) || ('guardContent' in c && c.guardContent?.text?.text) || ''
        )
        .join(' ')
        .trim()
    )
    .filter(Boolean)
}

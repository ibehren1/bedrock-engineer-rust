/**
 * The system-prompt block telling the agent where this chat's files belong.
 *
 * Every chat has one folder in the project (`chats/<chat-name>/`, named after the chat title).
 * Without this block the model invents a folder per task in the project root, which is how
 * files ended up scattered. The folder is created when something is first written to it.
 */

/**
 * The block for a chat whose folder is `directory` (absolute) / `relative` (project-relative,
 * `/`-separated). Placeholders are already resolved by the time this block is added, so it
 * carries real paths.
 */
export function chatFolderPrompt(relative: string, directory: string): string {
  if (!relative || !directory) return ''
  return `**<chat folder>**

This chat's folder is \`${directory}\`. Files attached to the chat are in \`${relative}/attachments/\`.

- Save anything you create for this chat there — code, notes, diagrams, generated images, exports. Keep whatever folder structure you like inside it.
- A relative path in the file tools, \`executeCommand\` and \`codeInterpreter\` already resolves to this folder, so \`notes.md\` means \`${relative}/notes.md\`. Reading a relative path that is not in the chat folder falls back to the project folder, so \`src/main.rs\` still reads the project's file.
- Do not create new folders of your own at the top of the project folder.
- Change files elsewhere in the project only when the user asks you to work on their project files, and use an absolute path when you do.

**</chat folder>**`
}

/** The chat folder block for `sessionId`, or `''` when there is no folder (no project set). */
export async function loadChatFolderPrompt(sessionId: string | undefined): Promise<string> {
  if (!sessionId) return ''
  try {
    const folder = await window.api.chatAttachments.folder(sessionId)
    return chatFolderPrompt(folder?.relative ?? '', folder?.directory ?? '')
  } catch (error) {
    console.warn('[chatFolder] could not resolve the chat folder', error)
    return ''
  }
}

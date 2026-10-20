import { createContext, useContext } from 'react'

/**
 * Lets chat content open a file in the chat page's Files panel. Provided by ChatPage; outside
 * it (other pages using the same Markdown renderer) there is no provider and file paths render
 * as plain text.
 */
export type FilesPanelContextValue = {
  projectPath: string
  /** Open the Files panel with this project-relative path selected. */
  openFile: (path: string, kind?: 'file' | 'dir') => void
  /** Bumped when project files change, so paths that were missing are looked up again. */
  filesVersion: number
}

export const FilesPanelContext = createContext<FilesPanelContextValue | null>(null)

export const useFilesPanel = (): FilesPanelContextValue | null => useContext(FilesPanelContext)

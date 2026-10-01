// `window.file`, installed by src/renderer/src/lib/tauriBridge.ts. Like the preload it replaced,
// these methods never reject: failures come back as `{ success: false, error }` /
// `{ agents: [], error }`. The `declare function`s below exist only to name the signatures.
import type { CustomAgent } from '../agent-chat'
declare function readSharedAgents(): Promise<{
  agents: CustomAgent[]
  error?: Error
}>
declare function readDirectoryAgents(): Promise<{
  agents: CustomAgent[]
  error?: Error
}>
/**
 * Save an agent as a shared agent to the project's .bedrock-engineer/agents directory
 * @param agent The agent to save
 * @param options Optional settings for saving (format)
 * @returns Result with success status and path/error details
 */
declare function saveSharedAgent(
  agent: CustomAgent,
  options?: {
    format?: 'json' | 'yaml'
  }
): Promise<{
  success: boolean
  filePath?: string
  format?: string
  error?: string
}>
/**
 * Delete the shared copy of an agent: its file under .bedrock-engineer/agents. The user confirms in
 * a native dialog first, and their own copy of the agent is left untouched.
 * @param filePath The shared agent file to delete, as reported by readSharedAgents
 * @returns Result with success status, or `canceled` when the user backed out
 */
declare function deleteSharedAgent(filePath: string): Promise<{
  success: boolean
  canceled?: boolean
  filePath?: string
  error?: string
}>
/**
 * Write an agent to a YAML file the user picks, for sending to someone else or keeping outside the
 * app. Fields describing this particular copy (id, sharing flags) are left out of the file.
 * @param agent The agent to export
 * @returns Result with the written path, or `canceled` when the user dismissed the save dialog
 */
declare function exportAgentYaml(agent: CustomAgent): Promise<{
  success: boolean
  canceled?: boolean
  filePath?: string
  error?: string
}>
/**
 * Read an agent config file the user picks. The caller is responsible for giving the result a fresh
 * id and adding it to the user's own agents.
 * @returns The parsed agent, or `canceled` when the user dismissed the file picker
 */
declare function importAgentFile(): Promise<{
  success: boolean
  canceled?: boolean
  agent?: CustomAgent
  filePath?: string
  error?: string
}>
/**
 * Load organization agents from S3
 * @param organizationConfig The organization configuration
 * @returns Result with agents and error details
 */
declare function loadOrganizationAgents(organizationConfig: any): Promise<{
  agents: CustomAgent[]
  error?: string
}>
/**
 * Save an agent to organization S3
 * @param agent The agent to save
 * @param organizationConfig The organization configuration
 * @param options Optional settings for saving (format)
 * @returns Result with success status and details
 */
declare function saveAgentToOrganization(
  agent: CustomAgent,
  organizationConfig: any,
  options?: {
    format?: 'json' | 'yaml'
  }
): Promise<{
  success: boolean
  s3Key?: string
  format?: string
  error?: string
}>
/**
 * Export a chat session to a standard markdown file under <projectPath>/<title>/.
 * Rendered diagrams/images are written as PNGs into an images/ subdirectory.
 * @param data The export title, markdown text, and base64-encoded PNG images
 * @returns Result with success status and the written file path / directory
 */
declare function exportChatMarkdown(data: {
  title: string
  markdown: string
  images: {
    filename: string
    base64: string
  }[]
}): Promise<{
  success: boolean
  filePath?: string
  directory?: string
  error?: string
}>
/**
 * Export a chat session to a Word (.docx) file under <projectPath>/<title>/.
 * The provided HTML (rich text, with inline base64 images) is converted to docx in the renderer (html-to-docx) and written by Rust.
 * @param data The export title and self-contained HTML body
 * @returns Result with success status and the written file path / directory
 */
declare function exportChatDocx(data: { title: string; html: string }): Promise<{
  success: boolean
  filePath?: string
  directory?: string
  error?: string
}>
/**
 * Export a chat session to a PDF file under <projectPath>/<title>/.
 * The provided HTML (with inline base64 images) is printed to PDF by the platform web view (src-tauri/app/src/pdf).
 * @param data The export title and self-contained HTML document
 * @returns Result with success status and the written file path / directory
 */
declare function exportChatPdf(data: { title: string; html: string }): Promise<{
  success: boolean
  filePath?: string
  directory?: string
  error?: string
}>
export type FileApi = {
  handleFolderOpen: () => Promise<any>
  handleFileOpen: () => Promise<any>
  readSharedAgents: typeof readSharedAgents
  readDirectoryAgents: typeof readDirectoryAgents
  saveSharedAgent: typeof saveSharedAgent
  deleteSharedAgent: typeof deleteSharedAgent
  exportAgentYaml: typeof exportAgentYaml
  importAgentFile: typeof importAgentFile
  loadOrganizationAgents: typeof loadOrganizationAgents
  saveAgentToOrganization: typeof saveAgentToOrganization
  exportChatMarkdown: typeof exportChatMarkdown
  exportChatDocx: typeof exportChatDocx
  exportChatPdf: typeof exportChatPdf
}

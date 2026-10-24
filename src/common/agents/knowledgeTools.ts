import type { Tool } from '@aws-sdk/client-bedrock-runtime'
import type { CustomAgent, McpServerConfig } from '../../types/agent-chat'

/**
 * The tools of the built-in `local-knowledgebase` MCP server, which every agent with local
 * knowledgebases gets. Same list as `src-tauri/crates/knowledge/data/tools.json` (the server's
 * own definitions; `knowledgeTools.test.ts` keeps the two in sync). Calls go through the MCP
 * path like any MCP tool; the backend attaches the server from the agent's
 * `localKnowledgeBases`.
 */
export const KNOWLEDGE_BASE_TOOL_DEFINITIONS = [
  {
    name: 'searchKnowledgeBase',
    description:
      "Search this agent's local knowledge bases (documents and websites the user added) and return the most relevant passages with their sources. Use it before answering questions the knowledge bases may cover, and cite the sources you use.",
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'What to look for. A question or key terms.' },
        knowledgeBase: {
          type: 'string',
          description: 'Search only this knowledge base (name or id). Omit to search all.'
        },
        topK: {
          type: 'integer',
          minimum: 1,
          maximum: 20,
          description: 'Number of passages to return.'
        }
      },
      required: ['query']
    }
  },
  {
    name: 'listKnowledgeBases',
    description: "List this agent's local knowledge bases with their descriptions and sizes.",
    inputSchema: { type: 'object', properties: {} }
  },
  {
    name: 'listKnowledgeBaseSources',
    description: "List the files and websites in this agent's local knowledge bases.",
    inputSchema: {
      type: 'object',
      properties: {
        knowledgeBase: {
          type: 'string',
          description: 'Only this knowledge base (name or id). Omit for all.'
        }
      }
    }
  }
] as const

export const KNOWLEDGE_BASE_TOOL_NAMES: readonly string[] = KNOWLEDGE_BASE_TOOL_DEFINITIONS.map(
  (t) => t.name
)

export const hasLocalKnowledgeBases = (agent?: Pick<CustomAgent, 'localKnowledgeBases'>): boolean =>
  (agent?.localKnowledgeBases?.length ?? 0) > 0

/** The knowledgebase tools as Bedrock tool specs, or `[]` for an agent without any. */
export function knowledgeBaseTools(agent?: Pick<CustomAgent, 'localKnowledgeBases'>): Tool[] {
  if (!hasLocalKnowledgeBases(agent)) return []
  return KNOWLEDGE_BASE_TOOL_DEFINITIONS.map((t) => ({
    toolSpec: {
      name: t.name,
      description: t.description,
      inputSchema: { json: JSON.parse(JSON.stringify(t.inputSchema)) }
    }
  }))
}

/** Name of the built-in server (`LOCAL_KNOWLEDGE_BASE_SERVER` in `crates/common`). */
export const LOCAL_KNOWLEDGE_BASE_SERVER = 'local-knowledgebase'

/** The built-in server entry for an agent (`local_knowledge_base_server` in `crates/common`). */
export const localKnowledgeBaseServer = (agentId: string): McpServerConfig => ({
  name: LOCAL_KNOWLEDGE_BASE_SERVER,
  description: "Searches this agent's local knowledge bases",
  connectionType: 'builtin',
  agentId
})

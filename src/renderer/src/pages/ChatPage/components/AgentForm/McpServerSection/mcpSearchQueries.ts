import { AgentMarketContext } from './mcpMarket'

/**
 * The model's only job here is to turn an agent's configuration into search
 * terms for the MCP Registry. The servers themselves come from the registry, so
 * no package name in the UI is ever model-invented.
 *
 * Terms have to be grounded: each one carries the phrase from the agent's own
 * configuration that justifies it, which keeps suggestions specific to this
 * agent instead of drifting into generic "automation"/"devops" territory.
 */
export type McpSearchQuery = {
  term: string
  /** The phrase from the agent's configuration this term came from */
  reason: string
}

export type McpSearchQueryResponse = {
  queries: McpSearchQuery[]
}

export const MCP_SEARCH_QUERY_TOOL = {
  name: 'suggest_mcp_search_terms',
  description: 'Propose grounded search terms for finding MCP servers that suit an agent'
}

export const mcpSearchQuerySchema = {
  type: 'object',
  properties: {
    queries: {
      type: 'array',
      description: 'Search terms for an MCP server directory, most relevant first',
      items: {
        type: 'object',
        properties: {
          term: {
            type: 'string',
            description:
              'One or two words naming a specific system, product, CLI or data source the agent works with (e.g. "postgres", "jira", "kubectl")'
          },
          reason: {
            type: 'string',
            description:
              "Short quote or paraphrase from the agent's configuration that this term comes from"
          }
        },
        required: ['term', 'reason']
      },
      minItems: 1,
      maxItems: 5
    }
  },
  required: ['queries']
}

/** Words too generic to search a server directory with. */
const BANNED_TERM_EXAMPLES = [
  'automation',
  'workflow',
  'devops',
  'deployment',
  'ci/cd',
  'monitoring',
  'productivity',
  'data',
  'api',
  'cloud',
  'ai',
  'assistant'
]

export const buildMcpSearchQuerySystemPrompt = (): string =>
  `You find Model Context Protocol (MCP) servers for one specific AI agent.

You are given that agent's configuration. Extract the concrete systems it needs to talk to and return
them as search terms with the 'suggest_mcp_search_terms' tool.

Rules:
- Every term must be grounded in the agent's configuration. Put the phrase it came from in "reason".
  If nothing in the configuration names a system, return fewer terms — one good term beats five
  guesses, and returning an empty list is better than inventing needs.
- Name the specific system, product, CLI or data source: "postgres", "jira", "kubectl", "grafana",
  "shopify", "figma", "snowflake". Prefer proper nouns the agent's own text uses.
- Reject generic category words; they match nothing useful in a server directory. Never return terms
  like: ${BANNED_TERM_EXAMPLES.join(', ')}.
- Read the scenarios and allowed commands closely — they name real tools (a command like
  "kubectl get pods" means "kubernetes"; "gh pr list" means "github").
- Skip anything the agent already covers with a built-in tool or an existing MCP server.
- One or two words per term. They are directory queries, not sentences.`

const truncate = (value: string, max: number) =>
  value.length > max ? `${value.slice(0, max)}…` : value

export const buildMcpSearchQueryUserMessage = (
  context: AgentMarketContext & { existingServerNames?: string[] }
): string => {
  const lines = [
    `Agent name: ${context.name?.trim() || '(unnamed)'}`,
    `Agent description: ${context.description?.trim() || '(none)'}`,
    `Agent category: ${context.category?.trim() || '(none)'}`,
    `Built-in tools already enabled: ${
      context.toolNames?.length ? context.toolNames.join(', ') : '(none)'
    }`,
    `MCP servers already configured: ${
      context.existingServerNames?.length ? context.existingServerNames.join(', ') : '(none)'
    }`
  ]

  const commands = (context.allowedCommands || []).filter(Boolean)
  if (commands.length > 0) {
    lines.push('', 'Allowed shell commands (these name the tools it drives):')
    commands.slice(0, 30).forEach((command) => lines.push(`- ${command}`))
  }

  const scenarios = (context.scenarios || []).filter(
    (scenario) => scenario?.title?.trim() || scenario?.content?.trim()
  )
  if (scenarios.length > 0) {
    lines.push('', 'Scenarios the agent is expected to handle:')
    scenarios.slice(0, 12).forEach((scenario) => {
      const title = scenario.title?.trim() || '(untitled)'
      const content = scenario.content?.trim()
      lines.push(`- ${title}${content ? `: ${truncate(content, 300)}` : ''}`)
    })
  }

  const additional = context.additionalInstruction?.trim()
  if (additional) {
    lines.push('', 'Additional instruction:', truncate(additional, 800))
  }

  const system = (context.system || '').trim()
  if (system) {
    // The system prompt is where the specifics live, so send a generous slice.
    lines.push('', 'System prompt:', truncate(system, 8000))
  }

  return lines.join('\n')
}

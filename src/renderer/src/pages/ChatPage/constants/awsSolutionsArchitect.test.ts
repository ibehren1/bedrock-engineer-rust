import { DEFAULT_AGENTS } from './DEFAULT_AGENTS'
import { AWS_SOLUTIONS_ARCHITECT_AGENT_ID } from './awsSolutionsArchitectAgent'
import { BUILT_IN_TOOLS } from '@/types/tools'
import { agent as agentStrings } from '@renderer/i18n/locales/chat/agent'
import { examples } from '@renderer/i18n/locales/chat/examples'

describe('the AWS Solutions Architect agent', () => {
  const architect = DEFAULT_AGENTS.find((agent) => agent.id === AWS_SOLUTIONS_ARCHITECT_AGENT_ID)

  it('ships as a built-in agent', () => {
    expect(architect).toBeDefined()
    expect(architect?.isCustom).toBe(false)
  })

  // The index-based prompt exports at the end of DEFAULT_AGENTS stop at the Svelte generator.
  it('does not shift the agents before it', () => {
    expect(DEFAULT_AGENTS[8].id).toBe('svelteGeneratorAgent')
  })

  it('only lists built-in tools (MCP tools come from its servers)', () => {
    for (const tool of architect?.tools ?? []) {
      expect(BUILT_IN_TOOLS).toContain(tool)
    }
  })

  // The Rust config check rejects a command server without args, which drops every server.
  it('gives every command server its args', () => {
    const servers = architect?.mcpServers ?? []
    expect(servers.length).toBeGreaterThan(0)
    for (const server of servers.filter((s) => s.connectionType === 'command')) {
      expect(server.command).toBeTruthy()
      expect(Array.isArray(server.args)).toBe(true)
    }
  })

  it('has unique server names', () => {
    const names = (architect?.mcpServers ?? []).map((s) => s.name)
    expect(new Set(names).size).toBe(names.length)
  })

  // Servers that call AWS get the app's credentials only through these placeholders.
  it('passes AWS settings only through placeholders', () => {
    for (const server of architect?.mcpServers ?? []) {
      for (const [key, value] of Object.entries(server.env ?? {})) {
        if (key === 'AWS_PROFILE') expect(value).toBe('{{awsProfile}}')
        if (key === 'AWS_REGION') expect(value).toBe('{{awsRegion}}')
      }
    }
  })

  it('uses the project path placeholder in its prompt', () => {
    expect(architect?.system).toContain('{{projectPath}}')
    expect(architect?.system).toContain('{{date}}')
  })

  it('has its description and scenarios translated', () => {
    for (const lang of ['en', 'ja'] as const) {
      expect(agentStrings[lang]).toHaveProperty([architect!.description])
      for (const { title } of architect?.scenarios ?? []) {
        expect(examples[lang]).toHaveProperty([title])
        expect(examples[lang]).toHaveProperty([`${title} description`])
      }
    }
  })
})

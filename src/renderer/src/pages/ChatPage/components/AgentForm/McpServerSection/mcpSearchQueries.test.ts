import { buildMcpSearchQuerySystemPrompt, buildMcpSearchQueryUserMessage } from './mcpSearchQueries'

describe('buildMcpSearchQueryUserMessage', () => {
  it('includes the specifics: scenarios, allowed commands and the system prompt', () => {
    const message = buildMcpSearchQueryUserMessage({
      name: 'Release Manager',
      description: 'Owns the weekly release',
      category: 'software',
      toolNames: ['executeCommand'],
      existingServerNames: ['github'],
      allowedCommands: ['kubectl *', 'psql *'],
      scenarios: [{ title: 'Check rollout', content: 'Confirm the canary in the staging cluster' }],
      additionalInstruction: 'Always mention the release ticket',
      system: 'You watch Grafana dashboards and query the orders database before sign-off.'
    })

    expect(message).toContain('Agent name: Release Manager')
    expect(message).toContain('Built-in tools already enabled: executeCommand')
    expect(message).toContain('MCP servers already configured: github')
    expect(message).toContain('- kubectl *')
    expect(message).toContain('- Check rollout: Confirm the canary in the staging cluster')
    expect(message).toContain('Always mention the release ticket')
    expect(message).toContain('You watch Grafana dashboards')
  })

  it('omits empty sections rather than sending placeholders', () => {
    const message = buildMcpSearchQueryUserMessage({ name: 'Bare', description: 'Nothing yet' })

    expect(message).not.toContain('Allowed shell commands')
    expect(message).not.toContain('Scenarios the agent')
    expect(message).not.toContain('System prompt:')
    expect(message).toContain('Agent category: (none)')
  })

  it('truncates a long system prompt instead of sending everything', () => {
    const message = buildMcpSearchQueryUserMessage({ system: 'x'.repeat(12000) })

    expect(message).toContain('…')
    expect(message.length).toBeLessThan(9000)
  })
})

describe('buildMcpSearchQuerySystemPrompt', () => {
  it('requires grounding and rejects generic category words', () => {
    const prompt = buildMcpSearchQuerySystemPrompt()

    expect(prompt).toContain('grounded')
    expect(prompt).toContain('devops')
    expect(prompt).toContain('ci/cd')
    expect(prompt).toContain('kubectl')
  })
})

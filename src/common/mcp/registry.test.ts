import {
  normalizeRegistryServers,
  registryLaunchLabel,
  registryServerLaunch,
  registryServerToConfigJson,
  registryShortName
} from './registry'

// Trimmed shape of a real /v0/servers?search=… response.
const payload = {
  servers: [
    {
      server: {
        name: 'io.github.containers/kubernetes-mcp-server',
        description: 'A Model Context Protocol (MCP) server for Kubernetes and OpenShift',
        version: '0.0.66',
        repository: {
          url: 'https://github.com/containers/kubernetes-mcp-server',
          source: 'github'
        },
        packages: [
          { registryType: 'npm', identifier: 'kubernetes-mcp-server', version: '0.0.66' },
          { registryType: 'pypi', identifier: 'kubernetes-mcp-server', version: '0.0.66' },
          { registryType: 'oci', identifier: 'ghcr.io/containers/kubernetes-mcp-server:v0.0.66' }
        ]
      }
    },
    {
      server: {
        name: 'ai.waystation/postgres',
        title: 'Postgres',
        description: 'Connect to your PostgreSQL database to query data and schemas.',
        remotes: [
          { type: 'streamable-http', url: 'https://waystation.ai/postgres/mcp' },
          { type: 'sse', url: 'https://waystation.ai/postgres/mcp/sse' }
        ]
      }
    },
    {
      server: {
        name: 'io.github.someone/pypi-only',
        description: 'Python only server',
        packages: [
          {
            registryType: 'pypi',
            identifier: 'some-mcp',
            environmentVariables: [{ name: 'API_KEY' }, { name: 'REGION' }]
          }
        ]
      }
    },
    {
      server: {
        name: 'io.github.someone/repo-only',
        description: 'Published without package metadata'
      }
    },
    { server: { description: 'nameless entries are dropped' } }
  ]
}

describe('normalizeRegistryServers', () => {
  it('maps names, descriptions, packages and remotes, dropping nameless entries', () => {
    const servers = normalizeRegistryServers(payload)

    expect(servers).toHaveLength(4)
    expect(servers[0]).toMatchObject({
      name: 'io.github.containers/kubernetes-mcp-server',
      shortName: 'kubernetes-mcp-server',
      version: '0.0.66',
      repositoryUrl: 'https://github.com/containers/kubernetes-mcp-server'
    })
    expect(servers[0].packages.map((pkg) => pkg.registryType)).toEqual(['npm', 'pypi', 'oci'])
    expect(servers[1].remotes[0]).toEqual({
      type: 'streamable-http',
      url: 'https://waystation.ai/postgres/mcp'
    })
  })

  it('collects environment variable names only', () => {
    const servers = normalizeRegistryServers(payload)
    expect(servers[2].packages[0].envVars).toEqual(['API_KEY', 'REGION'])
  })

  it('tolerates junk input', () => {
    expect(normalizeRegistryServers(undefined)).toEqual([])
    expect(normalizeRegistryServers({ servers: 'nope' })).toEqual([])
  })
})

describe('registryServerLaunch', () => {
  const servers = normalizeRegistryServers(payload)

  it('prefers the npm package and pins the version', () => {
    expect(registryServerLaunch(servers[0])).toEqual({
      kind: 'command',
      command: 'npx',
      args: ['-y', 'kubernetes-mcp-server@0.0.66'],
      envVars: []
    })
  })

  it('falls back to a remote endpoint when there is no package', () => {
    expect(registryServerLaunch(servers[1])).toEqual({
      kind: 'url',
      url: 'https://waystation.ai/postgres/mcp'
    })
  })

  it('uses uvx for pypi-only servers', () => {
    expect(registryServerLaunch(servers[2])).toMatchObject({ command: 'uvx', args: ['some-mcp'] })
  })

  it('returns null when the entry has no way to run', () => {
    expect(registryServerLaunch(servers[3])).toBeNull()
    expect(registryLaunchLabel(servers[3])).toBeNull()
  })
})

describe('registryServerToConfigJson', () => {
  const servers = normalizeRegistryServers(payload)

  it('builds a command config keyed by the short name', () => {
    expect(JSON.parse(registryServerToConfigJson(servers[0])!)).toEqual({
      mcpServers: {
        'kubernetes-mcp-server': {
          command: 'npx',
          args: ['-y', 'kubernetes-mcp-server@0.0.66']
        }
      }
    })
  })

  it('includes empty placeholders for required environment variables', () => {
    expect(JSON.parse(registryServerToConfigJson(servers[2])!)).toEqual({
      mcpServers: {
        'pypi-only': {
          command: 'uvx',
          args: ['some-mcp'],
          env: { API_KEY: '', REGION: '' }
        }
      }
    })
  })

  it('builds a url config for remote servers', () => {
    expect(JSON.parse(registryServerToConfigJson(servers[1])!)).toEqual({
      mcpServers: { postgres: { url: 'https://waystation.ai/postgres/mcp' } }
    })
  })

  it('returns null when there is nothing to launch', () => {
    expect(registryServerToConfigJson(servers[3])).toBeNull()
  })
})

describe('registryShortName', () => {
  it('takes the last segment and sanitises it', () => {
    expect(registryShortName('io.github.foo/bar-baz')).toBe('bar-baz')
    expect(registryShortName('weird name/with spaces')).toBe('with-spaces')
  })
})

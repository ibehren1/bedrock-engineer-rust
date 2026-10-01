/**
 * Client-side types and mapping for the official MCP Registry
 * (https://registry.modelcontextprotocol.io).
 *
 * The registry is a public, documented API listing published MCP servers with
 * their package identifiers, versions, required environment variables and remote
 * endpoints — enough to build a working server config without guessing.
 */

export const MCP_REGISTRY_BASE_URL = 'https://registry.modelcontextprotocol.io'

export type McpRegistryPackage = {
  /** npm, pypi, oci, nuget, … */
  registryType: string
  identifier: string
  version?: string
  runtimeHint?: string
  /** Names only; the registry never carries values */
  envVars: string[]
}

export type McpRegistryRemote = {
  /** streamable-http or sse */
  type: string
  url: string
}

export type McpRegistryServer = {
  /** Fully qualified registry name, e.g. io.github.containers/kubernetes-mcp-server */
  name: string
  /** Last path segment, used as the config key */
  shortName: string
  title?: string
  description: string
  version?: string
  repositoryUrl?: string
  packages: McpRegistryPackage[]
  remotes: McpRegistryRemote[]
}

const asArray = (value: unknown): any[] => (Array.isArray(value) ? value : [])

/** Last path segment of a registry name, sanitised for use as a config key. */
export const registryShortName = (name: string): string => {
  const tail = name.split('/').pop() || name
  return tail.replace(/[^a-zA-Z0-9._-]/g, '-')
}

/** Map a `/v0/servers` response into the fields the UI needs. */
export const normalizeRegistryServers = (payload: any): McpRegistryServer[] =>
  asArray(payload?.servers)
    .map((entry) => entry?.server)
    .filter((server) => server?.name && typeof server.name === 'string')
    .map((server) => ({
      name: server.name,
      shortName: registryShortName(server.name),
      title: server.title || undefined,
      description: server.description || '',
      version: server.version || undefined,
      repositoryUrl: server.repository?.url || undefined,
      packages: asArray(server.packages).map((pkg) => ({
        registryType: pkg?.registryType || pkg?.registry_type || 'unknown',
        identifier: pkg?.identifier || '',
        version: pkg?.version || undefined,
        runtimeHint: pkg?.runtimeHint || pkg?.runtime_hint || undefined,
        envVars: asArray(pkg?.environmentVariables || pkg?.environment_variables)
          .map((variable) => variable?.name)
          .filter((name): name is string => !!name)
      })),
      remotes: asArray(server.remotes)
        .filter((remote) => remote?.url)
        .map((remote) => ({ type: remote.type || 'streamable-http', url: remote.url }))
    }))

/**
 * How a registry entry would be launched. `null` when the entry only points at a
 * repository, which happens for servers published without package metadata.
 */
export type McpRegistryLaunch =
  | { kind: 'command'; command: string; args: string[]; envVars: string[] }
  | { kind: 'url'; url: string }
  | null

export const registryServerLaunch = (server: McpRegistryServer): McpRegistryLaunch => {
  const npm = server.packages.find((pkg) => pkg.registryType === 'npm' && pkg.identifier)
  if (npm) {
    const spec = npm.version ? `${npm.identifier}@${npm.version}` : npm.identifier
    return { kind: 'command', command: 'npx', args: ['-y', spec], envVars: npm.envVars }
  }

  const pypi = server.packages.find((pkg) => pkg.registryType === 'pypi' && pkg.identifier)
  if (pypi) {
    return { kind: 'command', command: 'uvx', args: [pypi.identifier], envVars: pypi.envVars }
  }

  const remote = server.remotes[0]
  if (remote) {
    return { kind: 'url', url: remote.url }
  }

  return null
}

/** One-line summary of how the server runs, for display. */
export const registryLaunchLabel = (server: McpRegistryServer): string | null => {
  const launch = registryServerLaunch(server)
  if (!launch) return null
  return launch.kind === 'url' ? launch.url : [launch.command, ...launch.args].join(' ')
}

/** The `mcpServers` JSON for a registry entry, ready to paste into the editor. */
export const registryServerToConfigJson = (server: McpRegistryServer): string | null => {
  const launch = registryServerLaunch(server)
  if (!launch) return null

  const entry: Record<string, unknown> =
    launch.kind === 'url' ? { url: launch.url } : { command: launch.command, args: launch.args }

  if (launch.kind === 'command' && launch.envVars.length > 0) {
    entry.env = launch.envVars.reduce<Record<string, string>>((env, key) => {
      env[key] = ''
      return env
    }, {})
  }

  return JSON.stringify({ mcpServers: { [server.shortName]: entry } }, null, 2)
}

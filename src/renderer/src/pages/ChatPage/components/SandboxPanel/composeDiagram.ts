import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'

/**
 * Build a Mermaid diagram of a sandbox's layout.
 *
 * Drawn from the sandbox metadata and the live container list rather than by parsing the
 * compose file: the metadata is already normalised (the compose file may be agent-authored
 * and shaped any number of ways), and the container list is what says whether each service
 * is actually up.
 *
 * Kept as a pure string builder so it can be tested without a browser.
 */

/** Mermaid node ids must be identifier-safe; service names are not. */
const nodeId = (prefix: string, name: string): string =>
  `${prefix}_${name.replace(/[^A-Za-z0-9_]/g, '_')}`

/** Quotes and brackets in a label would end the node early. */
const label = (text: string): string => text.replace(/["[\]{}]/g, '')

const shortenPath = (path: string): string => {
  const parts = path.split(/[/\\]/).filter(Boolean)
  return parts.length <= 2 ? path : `…/${parts.slice(-2).join('/')}`
}

export const buildComposeDiagram = (status: ChatSandboxStatus): string => {
  const services = status.metadata?.services ?? []
  const lines: string[] = ['graph LR']

  if (services.length === 0) {
    lines.push('  empty["No services"]')
    return lines.join('\n')
  }

  const hostPorts = services.flatMap((service) => service.ports.map((port) => port.host))

  // Host side: the browser entry points and the two mounts every sandbox has.
  lines.push('  subgraph host["Your machine"]')
  lines.push('    direction TB')
  if (hostPorts.length > 0) {
    lines.push(`    browser["Browser<br/>localhost"]`)
  }
  if (status.metadata?.projectPath) {
    lines.push(`    project["${label(shortenPath(status.metadata.projectPath))}"]`)
  }
  if (status.metadata?.directory) {
    lines.push(`    dataDir["${label(shortenPath(status.metadata.directory))}/data"]`)
  }
  lines.push('  end')

  lines.push(`  subgraph stack["${label(status.metadata?.projectName ?? 'sandbox')}"]`)
  lines.push('    direction TB')
  for (const service of services) {
    const id = nodeId('svc', service.name)
    const container = status.containers.find((item) => item.service === service.name)
    const state = (container?.state ?? 'stopped').toLowerCase()
    lines.push(
      `    ${id}["${label(service.name)}<br/>${label(service.image)}<br/>${label(state)}"]`
    )
  }
  lines.push('  end')

  for (const service of services) {
    const id = nodeId('svc', service.name)
    for (const port of service.ports) {
      lines.push(`  browser -->|"${port.host} → ${port.container}"| ${id}`)
    }
    if (status.metadata?.projectPath) {
      lines.push(`  project -.->|"/workspace"| ${id}`)
    }
    if (status.metadata?.directory) {
      lines.push(`  dataDir -.->|"/data"| ${id}`)
    }
  }

  // Services on one network can reach each other by name, which is the thing people most
  // often need reminding of in a compose stack.
  if (services.length > 1) {
    const [first, ...rest] = services
    for (const service of rest) {
      lines.push(
        `  ${nodeId('svc', first.name)} <-.->|"service name"| ${nodeId('svc', service.name)}`
      )
    }
  }

  for (const service of services) {
    const container = status.containers.find((item) => item.service === service.name)
    const up = (container?.state ?? '').toLowerCase() === 'running'
    lines.push(`  class ${nodeId('svc', service.name)} ${up ? 'up' : 'down'}`)
  }
  lines.push('  classDef up fill:#0b3b2e,stroke:#34d399,color:#d1fae5')
  lines.push('  classDef down fill:#3f3f46,stroke:#71717a,color:#e4e4e7')

  return lines.join('\n')
}

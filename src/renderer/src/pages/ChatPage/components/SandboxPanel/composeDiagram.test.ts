import { buildComposeDiagram } from './composeDiagram'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'

const status = (overrides: Partial<ChatSandboxStatus> = {}): ChatSandboxStatus => ({
  exists: true,
  state: 'running',
  metadata: {
    projectName: 'bedrock-sandbox-session-1',
    directory: '/Users/me/project/docker-sandboxes/fix-auth-a3f21c',
    projectPath: '/Users/me/project',
    composeless: false,
    services: [
      { name: 'main', image: 'ubuntu:26.04', ports: [{ host: 3000, container: 3000 }] },
      { name: 'db', image: 'postgres:17', ports: [] }
    ]
  },
  containers: [
    { service: 'main', name: 'bedrock-sandbox-session-1-main-1', state: 'running', ports: '' },
    { service: 'db', name: 'bedrock-sandbox-session-1-db-1', state: 'exited', ports: '' }
  ],
  ...overrides
})

describe('buildComposeDiagram', () => {
  it('draws a node per service with its image and state', () => {
    const diagram = buildComposeDiagram(status())

    expect(diagram).toContain('graph LR')
    expect(diagram).toContain('svc_main["main<br/>ubuntu:26.04<br/>running"]')
    expect(diagram).toContain('svc_db["db<br/>postgres:17<br/>exited"]')
  })

  it('links published ports from the browser to the service that owns them', () => {
    expect(buildComposeDiagram(status())).toContain('browser -->|"3000 → 3000"| svc_main')
  })

  it('omits the browser node when nothing is published', () => {
    const diagram = buildComposeDiagram(
      status({
        metadata: {
          ...status().metadata!,
          services: [{ name: 'main', image: 'ubuntu:26.04', ports: [] }]
        }
      })
    )
    expect(diagram).not.toContain('browser[')
  })

  it('shows both mounts every sandbox has', () => {
    const diagram = buildComposeDiagram(status())
    expect(diagram).toContain('project -.->|"/workspace"| svc_main')
    expect(diagram).toContain('dataDir -.->|"/data"| svc_main')
  })

  it('shortens long host paths so the diagram stays readable', () => {
    const diagram = buildComposeDiagram(status())
    expect(diagram).toContain('…/me/project')
    expect(diagram).not.toContain('/Users/me/project/docker-sandboxes/fix-auth-a3f21c"')
  })

  it('notes that services can reach each other by name, but only when there are several', () => {
    expect(buildComposeDiagram(status())).toContain('svc_main <-.->|"service name"| svc_db')

    const single = buildComposeDiagram(
      status({
        metadata: {
          ...status().metadata!,
          services: [{ name: 'main', image: 'ubuntu:26.04', ports: [] }]
        }
      })
    )
    expect(single).not.toContain('service name')
  })

  it('colours running and stopped services differently', () => {
    const diagram = buildComposeDiagram(status())
    expect(diagram).toContain('class svc_main up')
    expect(diagram).toContain('class svc_db down')
  })

  it('makes an identifier-safe node id from a service name Mermaid would choke on', () => {
    const diagram = buildComposeDiagram(
      status({
        metadata: {
          ...status().metadata!,
          services: [{ name: 'web-api.v2', image: 'node:22', ports: [] }]
        }
      })
    )
    expect(diagram).toContain('svc_web_api_v2[')
  })

  it('strips characters from a label that would end the node early', () => {
    const diagram = buildComposeDiagram(
      status({
        metadata: {
          ...status().metadata!,
          services: [{ name: 'main', image: 'registry/[weird]:tag"', ports: [] }]
        }
      })
    )
    expect(diagram).toContain('registry/weird:tag')
    expect(diagram.split('\n').filter((line) => line.includes('svc_main['))).toHaveLength(1)
  })

  it('says so rather than drawing nothing when there are no services', () => {
    const diagram = buildComposeDiagram({ exists: true, state: 'missing', containers: [] })
    expect(diagram).toContain('No services')
  })
})

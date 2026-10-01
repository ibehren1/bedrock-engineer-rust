/** Resource limits for the per-chat Docker sandbox (settings key `dockerSandboxTool`). */
export interface DockerSandboxConfig {
  memoryLimit: string
  cpuLimit: number
  /** Per-command timeout in seconds. */
  timeout: number
  /**
   * Set once the user has acknowledged what the interactive terminal is: a real root
   * shell with their project folder mounted read-write, subject to no allowlist.
   */
  terminalAcknowledged?: boolean
}

export const DEFAULT_SANDBOX_CONFIG: DockerSandboxConfig = {
  memoryLimit: '2g',
  cpuLimit: 2.0,
  timeout: 300
}

/** Container limits for the codeInterpreter tool (settings key `codeInterpreterTool`). */
export interface CodeInterpreterContainerConfig {
  memoryLimit: string // Memory limit for containers
  cpuLimit: number // CPU limit for containers
  timeout: number // Execution timeout in seconds
}

/**
 * Operating systems the sandbox can be built on. Mirrors `SANDBOX_OS_CATALOG` in
 * `src-tauri/crates/docker/src/types.rs`, which owns the images.
 */
export const SANDBOX_OS_OPTIONS = [
  { value: 'ubuntu', label: 'Ubuntu 26.04' },
  { value: 'debian13', label: 'Debian 13' },
  { value: 'amazonlinux2023', label: 'Amazon Linux 2023' },
  { value: 'amazonlinux2027', label: 'Amazon Linux 2027' },
  { value: 'almalinux9', label: 'AlmaLinux 9' },
  { value: 'rocky9', label: 'Rocky Linux 9' },
  { value: 'ubi9', label: 'Red Hat UBI 9' }
] as const

export type SandboxOsId = (typeof SANDBOX_OS_OPTIONS)[number]['value']

/** OS used when the settings name none; matches `DEFAULT_SANDBOX_OS_ID` in Rust. */
export const DEFAULT_SANDBOX_OS: SandboxOsId = 'debian13'

/** OS and resource limits for the per-chat Docker sandbox (settings key `dockerSandboxTool`). */
export interface DockerSandboxConfig {
  /** OS new sandboxes are built on. Unset means `DEFAULT_SANDBOX_OS`. */
  os?: SandboxOsId
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

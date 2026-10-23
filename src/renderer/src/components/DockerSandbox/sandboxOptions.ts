/** Choices for the Docker sandbox's container limits (settings key `dockerSandboxTool`). */

export const MEMORY_OPTIONS = [
  { value: '256m', label: '256 MB' },
  { value: '512m', label: '512 MB' },
  { value: '1g', label: '1 GB' },
  { value: '2g', label: '2 GB' },
  { value: '4g', label: '4 GB' },
  { value: '8g', label: '8 GB' }
]

export const CPU_OPTIONS = [
  { value: 0.5, label: '0.5 CPU (50%)' },
  { value: 1.0, label: '1.0 CPU (100%)' },
  { value: 2.0, label: '2.0 CPU (200%)' },
  { value: 4.0, label: '4.0 CPU (400%)' }
]

export const TIMEOUT_OPTIONS = [
  { value: 60, label: '60s' },
  { value: 300, label: '300s (5 min)' },
  { value: 600, label: '600s (10 min)' },
  { value: 1800, label: '1800s (30 min)' }
]

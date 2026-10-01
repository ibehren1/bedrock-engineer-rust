// `window.api`, installed by src/renderer/src/lib/tauriBridge.ts. Each method maps to a Tauri
// command in src-tauri/app/src/commands (table: docs/port/BRIDGE.md).
import type {
  ApplyGuardrailRequest,
  ApplyGuardrailCommandOutput,
  Tool
} from '@aws-sdk/client-bedrock-runtime'
import type { McpServerConfig } from '../agent-chat'
import type { McpRegistryServer } from '../../common/mcp/registry'
import type { ApplicationInferenceProfile, BedrockSupportRegion } from '../llm'
import type { IPCChannelDefinitions } from '../ipc'
import type { ToolResult } from '../tools'

export type API = {
  backgroundAgent: {
    chat: (params: {
      sessionId: string
      config: {
        modelId: string
        systemPrompt?: string
        agentId?: string
        projectDirectory?: string
        tools?: any[]
      }
      userMessage: string
      options?: any
    }) => Promise<any>
    onTaskNotification: (
      callback: (params: {
        taskId: string
        taskName: string
        success: boolean
        error?: string
        aiMessage?: string
        executedAt: number
        executionTime?: number
        sessionId?: string
        messageCount?: number
        toolExecutions?: number
        runCount?: number
        nextRun?: number
      }) => void
    ) => () => void
    onTaskExecutionStart: (
      callback: (params: { taskId: string; taskName: string; executedAt: number }) => void
    ) => () => void
    onTaskSkipped: (
      callback: (params: {
        taskId: string
        taskName: string
        reason: string
        executionTime?: number
      }) => void
    ) => () => void
    createSession: (
      sessionId: string,
      options?: {
        projectDirectory?: string
        agentId?: string
        modelId?: string
      }
    ) => Promise<any>
    deleteSession: (sessionId: string) => Promise<any>
    listSessions: () => Promise<any>
    getSessionHistory: (sessionId: string) => Promise<any>
    getSessionStats: (sessionId: string) => Promise<any>
    getAllSessionsMetadata: () => Promise<any>
    getSessionsByProject: (projectDirectory: string) => Promise<any>
    getSessionsByAgent: (agentId: string) => Promise<any>
    scheduleTask: (config: any) => Promise<any>
    updateTask: (taskId: string, config: any) => Promise<any>
    cancelTask: (taskId: string) => Promise<any>
    toggleTask: (taskId: string, enabled: boolean) => Promise<any>
    listTasks: () => Promise<any>
    getTask: (taskId: string) => Promise<any>
    getTaskExecutionHistory: (taskId: string) => Promise<any>
    executeTaskManually: (taskId: string) => Promise<any>
    getSchedulerStats: () => Promise<any>
    continueSession: (params: {
      sessionId: string
      taskId: string
      userMessage: string
      options?: {
        enableToolExecution?: boolean
        maxToolExecutions?: number
        timeoutMs?: number
      }
    }) => Promise<any>
    getTaskSystemPrompt: (taskId: string) => Promise<any>
  }
  bedrock: {
    executeTool: (toolInput: any, context?: any) => Promise<string | ToolResult<any>>
    applyGuardrail: (request: ApplyGuardrailRequest) => Promise<ApplyGuardrailCommandOutput>
    getImageGenerationModelsForRegion: (region: BedrockSupportRegion) => {
      id: string
      name: string
    }[]
    listApplicationInferenceProfiles: () => Promise<ApplicationInferenceProfile[]>
    convertInferenceProfileToLLM: (profile: any) => any
    translateText: (params: {
      text: string
      sourceLanguage?: string
      targetLanguage: string
      cacheKey?: string
    }) => Promise<any>
    translateBatch: (
      texts: Array<{
        text: string
        sourceLanguage?: string
        targetLanguage: string
      }>
    ) => Promise<any>
    getCachedTranslation: (params: {
      text: string
      sourceLanguage: string
      targetLanguage: string
    }) => Promise<any>
    clearTranslationCache: () => Promise<any>
    getTranslationCacheStats: () => Promise<any>
    getModelMaxTokens: (modelId: string) => Promise<any>
  }
  contextMenu: {
    onContextMenuCommand: (callback: (command: string) => void) => void
  }
  images: {
    getLocalImage: (path: string) => Promise<any>
  }
  openDirectory: () => Promise<any>
  readProjectIgnore: (projectPath: string) => Promise<any>
  writeProjectIgnore: (projectPath: string, content: string) => Promise<any>
  mcp: {
    init: (mcpServers: McpServerConfig[]) => Promise<any>
    getToolSpecs: (mcpServers: McpServerConfig[]) => Promise<any>
    executeTool: (toolName: string, input: any, mcpServers: McpServerConfig[]) => Promise<any>
    testConnection: (mcpServer: McpServerConfig) => Promise<any>
    testAllConnections: (mcpServers: McpServerConfig[]) => Promise<any>
    searchRegistry: (query: string, limit?: number) => Promise<McpRegistryServer[]>
    cleanup: () => Promise<any>
  }
  codeInterpreter: {
    getCurrentWorkspacePath: () => string | null
    checkDockerAvailability: () => Promise<any>
  }
  dockerSandbox: {
    availability: (force?: boolean) => Promise<any>
    create: (sessionId: string, options?: any) => Promise<any>
    status: (sessionId: string) => Promise<any>
    start: (sessionId: string) => Promise<any>
    stop: (sessionId: string) => Promise<any>
    remove: (
      sessionId: string,
      options?: {
        deleteData?: boolean
      }
    ) => Promise<any>
    rename: (sessionId: string) => Promise<any>
    logs: (
      sessionId: string,
      options?: {
        service?: string
        tail?: number
      }
    ) => Promise<any>
    exec: (sessionId: string, command: string, options?: any) => Promise<any>
    sendInput: (pid: number, stdin: string) => Promise<any>
    hasPid: (pid: number) => Promise<any>
    list: () => Promise<any>
    openFolder: (sessionId: string) => Promise<any>
    openPort: (sessionId: string, port: number) => Promise<any>
    insights: (sessionId: string, service?: string) => Promise<any>
    compose: (sessionId: string) => Promise<any>
    activity: (sessionId: string) => Promise<any>
    terminal: {
      capability: () => Promise<any>
      open: (
        sessionId: string,
        options?: {
          service?: string
          cols?: number
          rows?: number
        }
      ) => Promise<any>
      attach: (terminalId: string) => Promise<any>
      input: (terminalId: string, data: string) => Promise<any>
      resize: (terminalId: string, cols: number, rows: number) => Promise<any>
      backlog: (terminalId: string) => Promise<any>
      close: (terminalId: string) => Promise<any>
    }
  }
  chatAttachments: {
    list: (sessionId: string) => Promise<any>
    withFiles: (sessionIds: string[]) => Promise<any>
    add: (
      sessionId: string,
      files: {
        name: string
        bytes: Uint8Array
      }[]
    ) => Promise<any>
    addFromPicker: (sessionId: string) => Promise<any>
    remove: (sessionId: string, name: string) => Promise<any>
    removeAll: (sessionId: string) => Promise<any>
    removeEveryFolder: () => Promise<any>
    rename: (sessionId: string) => Promise<any>
    buildContext: (sessionId: string) => Promise<any>
    openFolder: (sessionId: string) => Promise<any>
  }
  help: {
    prepareUserGuide: (sessionId: string) => Promise<any>
  }
  screen: {
    listAvailableWindows: () => Promise<any>
  }
  camera: {
    saveCapturedImage: (request: {
      base64Data: string
      deviceId: string
      deviceName: string
      width: number
      height: number
      format: string
      outputPath?: string
    }) => Promise<any>
    showPreviewWindow: (options?: {
      size?: 'small' | 'medium' | 'large'
      opacity?: number
      position?: 'bottom-right' | 'bottom-left' | 'top-right' | 'top-left'
      cameraIds?: string[]
      layout?: 'cascade' | 'grid' | 'single'
    }) => Promise<any>
    hidePreviewWindow: () => Promise<any>
    closePreviewWindow: (deviceId: string) => Promise<any>
    updatePreviewSettings: (options: {
      size?: 'small' | 'medium' | 'large'
      opacity?: number
      position?: 'bottom-right' | 'bottom-left' | 'top-right' | 'top-left'
    }) => Promise<any>
    getPreviewStatus: () => Promise<any>
  }
  tools: {
    getToolSpecs: () => Tool[]
  }
  pubsub: {
    subscribe: (channel: string, callback: (data: any) => void) => () => void
    unsubscribe: (channel: string) => Promise<any>
    publish: (channel: string, data: any) => Promise<any>
    stats: () => Promise<any>
  }
  window: {
    isFocused: () => Promise<any>
    openTaskHistory: (taskId: string) => Promise<any>
  }
  todo: {
    getTodoList: (params?: { sessionId?: string }) => Promise<any>
    initTodoList: (params: { sessionId: string; items: string[] }) => Promise<any>
    updateTodoList: (params: {
      sessionId: string
      updates: Array<{
        id: string
        status?: 'pending' | 'in_progress' | 'completed' | 'cancelled'
        description?: string
      }>
    }) => Promise<any>
    deleteTodoList: (params: { sessionId: string }) => Promise<any>
    getRecentTodos: () => Promise<any>
    getAllTodoMetadata: () => Promise<any>
    setActiveTodoList: (params: { sessionId?: string }) => Promise<any>
    getActiveTodoListId: () => Promise<any>
  }
  strandsConverter: {
    convertAndSave: (agentId: string, outputDirectory: string) => Promise<any>
  }
  subAgent: {
    invoke: (
      params: IPCChannelDefinitions['sub-agent:invoke']['params']
    ) => Promise<IPCChannelDefinitions['sub-agent:invoke']['result']>
  }
}

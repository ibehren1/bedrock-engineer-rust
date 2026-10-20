// The `window.*` objects the renderer calls. src/renderer/src/lib/tauriBridge.ts installs them
// before the app loads, backed by Tauri commands and events (docs/port/BRIDGE.md).
import type { API } from './bridge/api'
import type { ChatHistoryApi } from './bridge/chat-history'
import type { FileApi } from './bridge/file'
import type { IpcClient } from './bridge/ipc'
import type { MemoryApi } from './bridge/memory'
import type { RendererCategoryLogger, RendererLogger } from './bridge/logger'
import type { ConfigStore } from './bridge/store'
import type { WorkspaceFilesApi } from './bridge/workspace-files'

declare global {
  interface Window {
    api: API
    store: ConfigStore
    file: FileApi
    chatHistory: ChatHistoryApi
    memory: MemoryApi
    workspaceFiles: WorkspaceFilesApi
    appWindow: {
      isFocused: () => Promise<boolean>
      /**
       * Ask `shouldClose` before this window closes; returning false keeps it open. Pending
       * store writes are flushed before an allowed close. Resolves to the unlisten function.
       */
      onCloseRequested: (shouldClose: () => boolean | Promise<boolean>) => Promise<() => void>
    }
    ipc: IpcClient
    logger: {
      log: RendererLogger
      createCategoryLogger: (category: string) => RendererCategoryLogger
    }
  }
}

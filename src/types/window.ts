// The `window.*` objects the renderer calls. src/renderer/src/lib/tauriBridge.ts installs them
// before the app loads, backed by Tauri commands and events (docs/port/BRIDGE.md).
import type { API } from './bridge/api'
import type { ChatHistoryApi } from './bridge/chat-history'
import type { FileApi } from './bridge/file'
import type { IpcClient } from './bridge/ipc'
import type { RendererCategoryLogger, RendererLogger } from './bridge/logger'
import type { ConfigStore } from './bridge/store'

declare global {
  interface Window {
    api: API
    store: ConfigStore
    file: FileApi
    chatHistory: ChatHistoryApi
    appWindow: {
      isFocused: () => Promise<boolean>
    }
    ipc: IpcClient
    logger: {
      log: RendererLogger
      createCategoryLogger: (category: string) => RendererCategoryLogger
    }
  }
}

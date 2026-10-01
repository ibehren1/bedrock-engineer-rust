// `window.ipc`: typed calls by channel name, mapped to Tauri commands by
// src/renderer/src/lib/tauriBridge.ts (docs/port/BRIDGE.md, rule 1).
import type { IPCChannels, IPCResult } from '../ipc'

export type IpcClient = {
  invoke: <C extends IPCChannels>(channel: C, ...args: any[]) => Promise<IPCResult<C>>
}

import { ToolName } from './tools'

// Planモードで実行可能なツール名のリスト
export const READ_ONLY_TOOLS: ToolName[] = [
  'readFiles',
  'listFiles',
  'tavilySearch',
  'fetchWebsite',
  'recognizeImage',
  'retrieve',
  'invokeBedrockAgent',
  'think',
  // Memory items are app settings, not project files, so the model may manage them while planning.
  'addMemory',
  'updateMemory',
  'removeMemory'
]

// ツールがPlanモードで使用可能かどうかを判定する関数
export const isPlanModeCompatible = (toolName: string): boolean => {
  return READ_ONLY_TOOLS.includes(toolName as ToolName)
}

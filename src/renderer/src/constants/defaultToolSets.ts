import { ToolState } from '@/types/agent-chat'

// カテゴリー毎のデフォルトツール設定（名前のみを定義）
const DEFAULT_TOOL_NAMES: Record<string, string[]> = {
  // 一般的な目的向け - 基本的なツールのみ
  general: ['readFiles', 'listFiles', 'tavilySearch', 'fetchWebsite', 'think'],

  // ソフトウェア開発向け - コード関連ツールを強化
  coding: [
    'createFolder',
    'writeToFile',
    'readFiles',
    'listFiles',
    'applyDiffEdit',
    'moveFile',
    'copyFile',
    'tavilySearch',
    'executeCommand',
    'invokeFlow',
    'think'
  ],

  // デザインとクリエイティブ向け
  design: ['readFiles', 'listFiles', 'fetchWebsite', 'tavilySearch', 'generateImage', 'think'],

  // データ分析向け
  data: [
    'readFiles',
    'listFiles',
    'tavilySearch',
    'fetchWebsite',
    'retrieve',
    'executeCommand',
    'think'
  ],

  // ビジネスと生産性向け
  business: ['readFiles', 'listFiles', 'tavilySearch', 'fetchWebsite', 'retrieve', 'think'],

  // ダイアグラム生成向け
  diagram: ['tavilySearch', 'think'],

  // ウェブサイト生成向け
  website: ['tavilySearch', 'retrieve', 'think'],

  all: [
    'createFolder',
    'writeToFile',
    'readFiles',
    'listFiles',
    'applyDiffEdit',
    'moveFile',
    'copyFile',
    'tavilySearch',
    'fetchWebsite',
    'retrieve',
    'generateImage',
    'invokeBedrockAgent',
    'executeCommand',
    'invokeFlow',
    'think'
  ],

  // カスタム設定用（すべて名前指定なし - 空配列）
  custom: []
}

// TODO: リージョンに応じて動的にツールの enum を設定したい
// "us-east-1",  "us-west-2", "ap-northeast-1" 以外は generateImage ツールを無効化する
const isGenerateImageTool = (name: string) => name === 'generateImage'

const supportGenerateImageToolRegions: string[] = [
  'us-east-1',
  'us-west-2',
  'ap-northeast-1',
  'eu-west-1',
  'eu-west-2',
  'ap-south-1'
]

const availableImageGenerationModelsMap: Record<string, string[]> = {
  'us-east-1': [
    'amazon.nova-canvas-v1:0',
    'amazon.titan-image-generator-v1',
    'amazon.titan-image-generator-v2:0'
  ],
  'us-west-2': [
    'stability.sd3-large-v1:0',
    'stability.sd3-5-large-v1:0',
    'stability.stable-image-core-v1:0',
    'stability.stable-image-core-v1:1',
    'stability.stable-image-ultra-v1:0',
    'stability.stable-image-ultra-v1:1',
    'amazon.titan-image-generator-v2:0',
    'amazon.titan-image-generator-v1'
  ],
  'ap-northeast-1': ['amazon.titan-image-generator-v2:0', 'amazon.titan-image-generator-v1'],
  'ap-south-1': ['amazon.titan-image-generator-v1'],
  'eu-west-1': ['amazon.titan-image-generator-v1'],
  'eu-west-2': ['amazon.titan-image-generator-v1']
}

/**
 * ツールの入力スキーマを更新する関数
 * generateImage ツールの modelId を現在のリージョンに対応するものに更新する
 *
 * @param tool 更新対象のツール
 * @param region 現在のリージョン
 * @returns 更新後のツール
 */
export const updateToolInputSchema = (tool: ToolState, region: string): ToolState => {
  if (tool.toolSpec?.name && isGenerateImageTool(tool.toolSpec.name)) {
    if (supportGenerateImageToolRegions.includes(region)) {
      const models = availableImageGenerationModelsMap[region] || []
      if (models.length === 0) return tool

      return {
        ...tool,
        toolSpec: {
          ...tool.toolSpec,
          inputSchema: {
            ...tool.toolSpec.inputSchema,
            json: {
              ...(tool.toolSpec.inputSchema?.json as any),
              properties: {
                ...(tool.toolSpec.inputSchema?.json as any).properties,
                modelId: {
                  ...((tool.toolSpec.inputSchema?.json as any).properties.modelId as any),
                  enum: models,
                  default: models[0]
                }
              }
            }
          }
        }
      }
    }
  }
  return tool
}

/**
 * カテゴリに基づいてツール設定を取得するヘルパー関数
 *
 * @param category エージェントカテゴリ
 * @param allTools 利用可能なすべてのツール
 * @returns カテゴリに基づいて設定されたツールリスト
 */
export const getToolsForCategory = (category: string, allTools: ToolState[]): ToolState[] => {
  const categoryNames = DEFAULT_TOOL_NAMES[category] || []

  // 現在のリージョンを取得
  const awsConfig = window.store.get('aws')
  const currentRegion = awsConfig?.region || ''

  // カテゴリに基づいてツール設定を生成
  const result = allTools.map((tool) => {
    const toolName = tool.toolSpec?.name
    if (!toolName) return { ...tool, enabled: false }

    // generateImage ツールの場合はリージョン対応をチェックし、入力スキーマも更新
    if (isGenerateImageTool(toolName)) {
      const isRegionSupported = supportGenerateImageToolRegions.includes(currentRegion)
      const updatedTool = isRegionSupported ? updateToolInputSchema(tool, currentRegion) : tool

      return {
        ...updatedTool,
        enabled: categoryNames.includes(toolName) && isRegionSupported
      }
    }

    // このツールがカテゴリに含まれるかどうか
    const isEnabled = categoryNames.includes(toolName)

    return {
      ...tool,
      enabled: isEnabled
    }
  })

  return result
}

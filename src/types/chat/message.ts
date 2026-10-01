import {
  Message as BedrockMessage,
  ConverseStreamMetadataEvent
} from '@aws-sdk/client-bedrock-runtime'

/**
 * IDを持つメッセージ型
 * AWS Bedrockのメッセージ型を拡張して、メッセージIDとメタデータを追加
 */
export interface IdentifiableMessage extends BedrockMessage {
  id?: string
  status?: 'idle' | 'streaming' | 'complete' | 'error'
  timestamp?: number
  metadata?: {
    converseMetadata?: ConverseStreamMetadataEvent | Record<string, any>
    sessionCost?: number
    // メッセージを生成したモデルのID（アシスタントメッセージのアイコン表示に使用）
    modelId?: string
    // 将来的に他のメタデータタイプも追加可能
  }
}

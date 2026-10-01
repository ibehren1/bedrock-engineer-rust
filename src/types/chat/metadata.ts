/**
 * メッセージIDとメタデータIDを生成するための関数
 */
export const generateMessageId = (): string =>
  `msg_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`

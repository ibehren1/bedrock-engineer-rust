import { LLM } from '@/types/llm'
import { useSettings } from '@renderer/contexts/SettingsContext'

/**
 * 軽量処理用のモデルIDを取得する
 * 軽量モデルが設定されていない場合はフォールバックモデルを返す
 */
export function getLightProcessingModelId(
  currentLLM: LLM,
  lightProcessingModel: LLM | null
): string {
  // 軽量処理用モデルが設定されている場合はそれを返す
  if (lightProcessingModel && lightProcessingModel.modelId) {
    return lightProcessingModel.modelId
  }

  // 設定されていない場合は現在のモデルを使用する
  return currentLLM?.modelId
}

/**
 * 軽量処理用モデルを使用するためのカスタムフック
 */
export function useLightProcessingModel() {
  const { currentLLM, lightProcessingModel } = useSettings()

  return {
    getLightModelId: () => getLightProcessingModelId(currentLLM, lightProcessingModel)
  }
}

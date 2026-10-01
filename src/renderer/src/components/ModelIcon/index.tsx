import { LuBrainCircuit } from 'react-icons/lu'
import NovaLogo from './nova-color.svg'
import ClaudeLogo from './claude-color.svg'
import DeepSeekLogo from './deepseek-color.svg'
import MetaLogo from './meta-color.svg'
import OpenAILogo from './openai-color.svg'
import XaiLogo from './xai-color.svg'

/**
 * 横長のワードマーク（xAI など）は正方形のスロットに入れると小さく潰れる。
 * 該当するモデルには {@link isWideModelIcon} で幅の広いスロットを与える。
 *
 * Width-to-height ratio of the wide wordmarks. Callers use it to size a slot
 * that the mark fills, instead of letterboxing it into a square one.
 */
export const WIDE_ICON_ASPECT = 2.62

/**
 * Whether a model's icon is a wide wordmark rather than a square glyph. Wide
 * marks are sized 100% in their own SVG so the slot's dimensions decide how big
 * they render; every other icon carries its own `1em` box.
 */
export const isWideModelIcon = (modelId: string): boolean =>
  modelId.includes('xai') || modelId.includes('grok')

/**
 * モデルIDからアイコンを返す。モデルセレクターの一覧とチャットのアバターで共有し、
 * 両者のアイコンが常に一致するようにする。
 * 専用アイコンが無いモデルは汎用アイコン（LuBrainCircuit）にフォールバックする。
 */
export const getModelIcon = (modelId: string, isInferenceProfile?: boolean) => {
  // Show group icon for inference profiles
  if (isInferenceProfile) return <LuBrainCircuit className="size-4 text-accent" />

  if (modelId.includes('claude')) return <ClaudeLogo />
  if (modelId.includes('nova')) return <NovaLogo />
  if (modelId.includes('deepseek')) return <DeepSeekLogo />
  if (modelId.includes('meta')) return <MetaLogo />
  if (modelId.includes('gpt-oss') || modelId.includes('openai')) return <OpenAILogo />
  if (isWideModelIcon(modelId)) return <XaiLogo />
  if (modelId.includes('llama')) return <LuBrainCircuit className="size-4" />
  return <LuBrainCircuit />
}

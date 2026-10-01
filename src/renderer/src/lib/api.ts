import { LLM } from '@/types/llm'
import type { RetrieveAndGenerateCommandInput } from '@aws-sdk/client-bedrock-agent-runtime'
import type {
  ConverseStreamOutput,
  InferenceConfiguration,
  Message,
  ToolConfiguration
} from '@aws-sdk/client-bedrock-runtime'
import { supportsStreamingWithToolUse } from '@common/models/models'
import { call, tauriConverse, tauriConverseStream, withAbort } from './tauriBridge'

export type StreamChatCompletionProps = {
  modelId: string
  system: { text: string }[] | undefined
  messages: Message[]
  toolConfig?: ToolConfiguration
}

// These invoke Rust commands (contract: docs/port/BRIDGE.md, "Converse").

/**
 * The stream events equivalent to a non-streaming `converse` result, for models that can't
 * stream with tool use. Each content block gets its own `contentBlockIndex`, and a tool use's
 * input arrives as a JSON-string delta, as in a real stream (consumers accumulate the deltas and
 * ignore `start.toolUse.input`).
 */
export function* converseOutputToStreamEvents(result: any): Generator<ConverseStreamOutput> {
  yield { messageStart: { role: result.output.message.role } }

  let index = 0
  for (const content of result.output.message.content || []) {
    if (content.text) {
      // テキストコンテンツを一度に出力
      yield { contentBlockStart: { start: undefined, contentBlockIndex: index } }
      yield { contentBlockDelta: { delta: { text: content.text }, contentBlockIndex: index } }
      yield { contentBlockStop: { contentBlockIndex: index } }
      index++
    } else if (content.toolUse) {
      // Tool Useコンテンツを出力
      const { toolUseId, name, input } = content.toolUse
      yield {
        contentBlockStart: { start: { toolUse: { toolUseId, name } }, contentBlockIndex: index }
      }
      yield {
        contentBlockDelta: {
          delta: { toolUse: { input: JSON.stringify(input ?? {}) } },
          contentBlockIndex: index
        }
      }
      yield { contentBlockStop: { contentBlockIndex: index } }
      index++
    }
  }

  yield { messageStop: { stopReason: result.stopReason } }

  if (result.usage) {
    yield { metadata: { usage: result.usage, metrics: { latencyMs: 0 } } }
  }
}

export async function* streamChatCompletion(
  props: StreamChatCompletionProps,
  abortSignal?: AbortSignal
): AsyncGenerator<ConverseStreamOutput, void, unknown> {
  const hasToolUse = props.toolConfig && props.toolConfig.tools && props.toolConfig.tools.length > 0

  // モデルがストリーミング + Tool Use をサポートしていない場合
  if (hasToolUse && !supportsStreamingWithToolUse(props.modelId)) {
    // 非ストリーミングAPIを使用し、結果をストリーミング形式に変換
    const result = await converse(props, abortSignal)

    // 非ストリーミング結果をストリーミング形式に変換
    yield* converseOutputToStreamEvents(result)
    return
  }

  yield* tauriConverseStream<ConverseStreamOutput>(props, abortSignal)
}

type ConverseProps = {
  modelId: string
  system: { text: string }[] | undefined
  messages: Message[]
  toolConfig?: ToolConfiguration
  /**
   * 指定しない場合、グローバル設定値を使用
   */
  inferenceConfig?: InferenceConfiguration
  /**
   * Disable extended thinking for this request regardless of the global setting.
   * Used by lightweight calls such as title generation.
   */
  disableThinking?: boolean
}

export async function converse(props: ConverseProps, abortSignal?: AbortSignal) {
  return tauriConverse<any>(props, abortSignal)
}

export async function retrieveAndGenerate(
  props: RetrieveAndGenerateCommandInput,
  abortSignal?: AbortSignal
) {
  // Callers get a Response, as from fetch (this used to be an HTTP route).
  const result = await withAbort(
    call('retrieveAndGenerate', 'retrieve_and_generate', { request: props }),
    abortSignal
  )
  return new Response(JSON.stringify(result), {
    status: 200,
    headers: { 'Content-Type': 'application/json' }
  })
}

export async function listModels(): Promise<LLM[]> {
  return call<LLM[]>('listModels', 'list_models')
}

export async function listAgentTags(): Promise<string[]> {
  return call<string[]>('listAgentTags', 'list_agent_tags')
}

/**
 * Get structured output from LLM using JSON schema
 * @param params - Request parameters including model, prompts, and output schema
 * @returns Structured output conforming to the provided schema
 */
export async function getStructuredOutput<T>(params: {
  modelId: string
  systemPrompt: string
  userMessage: string
  outputSchema: any
  toolOptions?: {
    name?: string
    description?: string
  }
  inferenceConfig?: InferenceConfiguration
}): Promise<T> {
  try {
    return await call<T>('getStructuredOutput', 'get_structured_output', { request: params })
  } catch (e: any) {
    throw new Error(`Structured output request failed: ${e?.message}`)
  }
}

/**
 * Get website improvement recommendations
 * @param params - Website code, language, and model ID
 * @returns Recommendations for website improvements
 */
export async function getWebsiteRecommendations(params: {
  websiteCode: string
  language: string
  modelId: string
}): Promise<{ recommendations: Array<{ title: string; value: string }> }> {
  try {
    return await call('getWebsiteRecommendations', 'get_website_recommendations', {
      request: params
    })
  } catch (e: any) {
    throw new Error(`Website recommendations request failed: ${e?.message}`)
  }
}

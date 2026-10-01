import { converse } from '@renderer/lib/api'
import toast from 'react-hot-toast'
import { sanitizeGeneratedTitle } from './sanitizeTitle'

export async function generateSessionTitle(
  session: { id: string; messages: any[] },
  modelId: string,
  t: any
): Promise<string | null> {
  // エラーログで使用するため、関数スコープで宣言
  let joinedMsgs = ''

  try {
    if (!session) {
      throw new Error('Session not found')
    }

    // チャット履歴から会話内容を取得
    const recentMessages = session.messages || []

    // セッションにメッセージがない場合は早期リターン
    if (recentMessages.length === 0) {
      console.warn('No messages found in session for title generation')
      return null
    }

    const messages = recentMessages.map((m) => ({
      role: m.role,
      content: m.content
    }))

    // The rules are explicit about plain text because models summarising a
    // technical conversation otherwise reach for `backticks` and **bold**, and a
    // title is rendered as raw text in the sidebar and reused as a folder name.
    // sanitizeGeneratedTitle() below is the guarantee; this is just the request.
    //
    // The previous version asked for "up to 15 characters", which is unreachable
    // in English, and had a missing newline that ran two rules together into one
    // unreadable line.
    const system = [
      {
        text:
          'You name chat conversations. Reply with nothing but the title.\n' +
          '\n' +
          'Rules:\n' +
          '- Plain text only. No Markdown, no backticks, no asterisks, no quotes, no heading marks.\n' +
          '- A short label, not a sentence. No trailing full stop.\n' +
          '- At most 6 words, or about 15 characters for Japanese and Chinese.\n' +
          '- Name the subject of the conversation, not the fact that it is a conversation.\n' +
          '- No preamble, no explanation, no "Title:" prefix.\n' +
          '\n' +
          'Good: Monthly sales summary\n' +
          'Good: Fixing the failing build\n' +
          'Bad: **Monthly Sales Summary**\n' +
          'Bad: "A conversation about monthly sales."\n' +
          'Bad: Title: Monthly sales summary'
      }
    ]

    // messages の配列に含まれるテキスト要素を結合する（上限 1000 文字）
    // textプロパティが存在するブロックのみを抽出
    joinedMsgs = messages
      .map((m) =>
        m.content
          ?.filter((v) => 'text' in v && v.text) // textプロパティがある場合のみ
          .map((v) => v.text)
          .join('\n')
      )
      .filter(Boolean) // 空文字列を除外
      .join('\n')
      .slice(0, 1000)

    // テキストコンテンツが空の場合は早期リターン
    if (!joinedMsgs.trim()) {
      console.warn('No text content found in messages for title generation')
      return null
    }

    // 軽量処理用モデルまたは現在のモデルを使用
    // modelIdはuseLightProcessingModelから取得されたものを使用
    // タイトル生成では thinking は不要なため無効化する
    // （有効なままだと max_tokens > thinking.budget_tokens 制約に違反してしまう）
    const response = await converse({
      modelId: modelId,
      system,
      disableThinking: true,
      inferenceConfig: {
        // A title is a handful of words. The old 4096 left room for the model to
        // return a paragraph, which then had to be salvaged.
        maxTokens: 64,
        temperature: 0.2
      },
      messages: [
        {
          role: 'user',
          content: [
            {
              text: joinedMsgs
            }
          ]
        }
      ]
    })

    // エラーレスポンス（output が存在しない）を分かりやすく扱う
    // converse() は HTTP エラー時もシリアライズされたエラーオブジェクトを返すため、
    // output を直接参照すると分かりにくい例外になる
    const message = response?.output?.message
    if (!message || !Array.isArray(message.content)) {
      const serverError =
        typeof response?.message === 'string' ? response.message : 'No output in response'
      throw new Error(serverError)
    }

    // レスポンスからテキスト要素のみを抽出
    const textContent = message.content.find((item) => 'text' in item)
    if (!textContent || !('text' in textContent)) {
      console.warn('No text content found in response:', response)
      return null
    }

    // Never trust the raw string: the prompt asks for plain text, but only this
    // guarantees it. Returns null when nothing usable is left, and the caller
    // keeps the existing default title.
    const title = sanitizeGeneratedTitle(textContent.text)
    if (!title) {
      console.warn('Generated title was empty after sanitising:', textContent.text)
      return null
    }
    return title
  } catch (error: any) {
    // エラーの詳細をログ出力
    console.error('Failed to generate AI title:', {
      error: error.message || error,
      errorDetails: error,
      modelId,
      sessionId: session.id,
      messageCount: session.messages?.length,
      hasTextContent: !!joinedMsgs?.trim()
    })

    // ユーザーには簡潔なエラーメッセージを表示
    toast.error(t('Failed to generate title'))
    return null
  }
}

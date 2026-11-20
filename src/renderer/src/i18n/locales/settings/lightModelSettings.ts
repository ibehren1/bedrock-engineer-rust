export const lightModelSettings = {
  en: {
    'settings.lightModel.useMain': 'Same as default model',

    'settings.defaultModels.title': 'Default Models',
    'settings.defaultModels.mainLabel': 'Default model',
    'settings.defaultModels.mainDescription':
      'Answers your chats and runs the Website, Diagram and Step Functions generators. New background agent sessions start with it too. You can switch models in a chat at any time.',
    'settings.defaultModels.lightLabel': 'Light processing model',
    'settings.defaultModels.lightDescription':
      'Handles small background jobs: naming chats, the Help chat, suggestions in the generators and MCP registry searches. A small, fast model keeps these cheap; leave it on the default model to use that instead.',

    'settings.thinking.title': 'Thinking Mode',
    'settings.thinking.description':
      'Lets the model reason before it answers. More thinking helps with hard, multi-step problems but takes longer and costs more, because thinking tokens are billed as output.',
    'settings.thinking.affects':
      'Applies to every conversation with the default model and with any model you pick in a chat (the brain icon in the chat changes this same setting). It is skipped for chat titles and for calls that must return structured output. Claude Fable 5 always thinks adaptively, and OpenAI GPT-6 and xAI Grok models turn the budget into a reasoning effort.',
    'settings.thinking.notSupported':
      'The default model ({{model}}) does not support thinking, so this setting has no effect until you choose a model that does.'
  },
  ja: {
    'settings.lightModel.useMain': 'デフォルトモデルと同じ',

    'settings.defaultModels.title': 'デフォルトモデル',
    'settings.defaultModels.mainLabel': 'デフォルトモデル',
    'settings.defaultModels.mainDescription':
      'チャットへの回答と、ウェブサイト・ダイアグラム・Step Functions ジェネレーターに使用します。新しいバックグラウンドエージェントのセッションもこのモデルで始まります。チャット中はいつでもモデルを切り替えられます。',
    'settings.defaultModels.lightLabel': '軽量処理用モデル',
    'settings.defaultModels.lightDescription':
      'チャットのタイトル付け、ヘルプチャット、ジェネレーターの提案、MCP レジストリ検索などの小さな処理に使用します。小さく速いモデルにするとコストを抑えられます。デフォルトモデルのままにすると、そのモデルを使用します。',

    'settings.thinking.title': '拡張思考モード',
    'settings.thinking.description':
      '回答の前にモデルに推論させます。思考を増やすと難しい多段階の問題に強くなりますが、思考トークンは出力として課金されるため、時間とコストが増えます。',
    'settings.thinking.affects':
      'デフォルトモデルと、チャットで選んだモデルとのすべての会話に適用されます（チャットの脳アイコンも同じ設定を変更します）。チャットタイトルの生成と構造化出力を返す呼び出しでは使用されません。Claude Fable 5 は常に適応的に思考し、OpenAI GPT-6 と xAI Grok ではバジェットが推論の強さに変換されます。',
    'settings.thinking.notSupported':
      'デフォルトモデル（{{model}}）は思考に対応していないため、対応モデルを選ぶまでこの設定は効果がありません。'
  }
}

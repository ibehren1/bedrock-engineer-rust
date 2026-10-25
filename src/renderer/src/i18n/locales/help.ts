export const help = {
  en: {
    help: {
      sidebarTooltip: 'Help',
      chatTitle: 'Bedrock Engineer Help',
      guideInPromptNotice:
        'No project directory is set, so the user guide is not shown as an attachment. Help still has the full guide.',
      guideFailed: 'The user guide could not be loaded: {{error}}',
      page: {
        title: 'User guide',
        contents: 'Contents',
        search: 'Search the guide',
        noResults: 'No matching sections.',
        ask: 'Ask a question',
        askHint: 'Chat with Help, answered from this guide',
        needsBedrock:
          'Asking questions needs Amazon Bedrock access, which isn’t working yet. The guide works without it.',
        setUpAccess: 'Setting up access',
        loadFailed: 'The user guide could not be loaded: {{error}}'
      },
      chat: {
        title: 'Ask Help',
        new: 'New',
        close: 'Close the chat',
        empty:
          'Ask how something in the app works. Answers come from this guide and link to the section they used.',
        placeholder: 'Ask a question…',
        send: 'Send',
        stop: 'Stop',
        thinking: 'Looking it up…'
      },
      excerptsFailed:
        'The guide index could not be searched, so Help is using the whole guide for this answer.'
    }
  },
  ja: {
    help: {
      sidebarTooltip: 'ヘルプ',
      chatTitle: 'Bedrock Engineer ヘルプ',
      guideInPromptNotice:
        'プロジェクトディレクトリが設定されていないため、ユーザーガイドは添付ファイルとして表示されません。ヘルプはガイド全体を参照できます。',
      guideFailed: 'ユーザーガイドを読み込めませんでした: {{error}}',
      page: {
        title: 'ユーザーガイド',
        contents: '目次',
        search: 'ガイドを検索',
        noResults: '一致するセクションはありません。',
        ask: '質問する',
        askHint: 'このガイドをもとにヘルプがチャットで答えます',
        needsBedrock:
          '質問するには Amazon Bedrock へのアクセスが必要ですが、まだ使える状態ではありません。ガイドはそのまま読めます。',
        setUpAccess: 'アクセスの設定',
        loadFailed: 'ユーザーガイドを読み込めませんでした: {{error}}'
      },
      chat: {
        title: 'ヘルプに質問',
        new: '新規',
        close: 'チャットを閉じる',
        empty:
          'アプリの使い方を質問してください。回答はこのガイドをもとにし、使ったセクションへのリンクが付きます。',
        placeholder: '質問を入力…',
        send: '送信',
        stop: '停止',
        thinking: '調べています…'
      },
      excerptsFailed:
        'ガイドのインデックスを検索できなかったため、この回答ではガイド全体を使っています。'
    }
  }
}

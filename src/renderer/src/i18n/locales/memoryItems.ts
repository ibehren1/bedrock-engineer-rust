export const memoryItems = {
  en: {
    memoryItems: {
      title: 'Memory',
      description:
        "Items saved here are added to every chat's system prompt while the brain button above the message box is on.",
      hint: 'In a chat, say "remember…", "add a rule…" or "forget…" and the assistant updates this list.',
      addPlaceholder: 'Add a memory or rule…',
      add: 'Add',
      kindLabel: 'Type',
      kind: {
        rule: 'rule',
        memory: 'memory'
      },
      kindOption: {
        rule: 'Rule',
        memory: 'Memory'
      },
      kindDescription: {
        rule: 'An instruction the assistant always follows, unless you tell it otherwise in that chat. For example: "Use pnpm instead of npm".',
        memory:
          'A fact the assistant keeps in mind and uses when it is relevant. It is background, not an instruction. For example: "The main repo is at ~/work/app".'
      },
      toggleKind: 'Click to switch between rule and memory',
      source: {
        chat: 'from chat',
        manual: 'added manually'
      },
      includeInContext: 'Include in context',
      edit: 'Edit',
      save: 'Save',
      cancel: 'Cancel',
      delete: 'Delete',
      confirmDelete: 'Delete this memory item?',
      editHint: 'Enter to save, Shift+Enter for a new line, Esc to cancel',
      count: '{{enabled}} of {{total}} enabled',
      empty: {
        title: 'No memory items yet',
        description:
          'Add one above, or ask the assistant in a chat to "remember…" or "add a rule…".'
      },
      loading: 'Loading memory items…',
      error: 'Memory items could not be saved: {{error}}',
      useMemoryItems: 'Use Memory Items',
      toggleOnTooltip: 'Memory items: on ({{count}} added to this chat)',
      toggleOffTooltip: 'Memory items: off for this chat',
      noSession: 'Available once the chat has started',
      chip: {
        saved: 'Memory saved',
        updated: 'Memory updated',
        removed: 'Memory removed',
        view: 'View',
        undo: 'Undo',
        undone: 'Undone'
      }
    }
  },
  ja: {
    memoryItems: {
      title: 'メモリ',
      description:
        'メッセージ入力欄の上にある脳のボタンがオンのとき、ここに保存した項目がすべてのチャットのシステムプロンプトに追加されます。',
      hint: 'チャットで「覚えておいて」「ルールを追加して」「忘れて」と伝えると、アシスタントがこの一覧を更新します。',
      addPlaceholder: 'メモリまたはルールを追加…',
      add: '追加',
      kindLabel: '種類',
      kind: {
        rule: 'ルール',
        memory: 'メモリ'
      },
      kindOption: {
        rule: 'ルール',
        memory: 'メモリ'
      },
      kindDescription: {
        rule: 'アシスタントが常に従う指示です。そのチャットで別の指示をした場合を除きます。例:「npm ではなく pnpm を使う」',
        memory:
          'アシスタントが念頭に置き、関係があるときに使う事実です。指示ではなく背景情報として扱われます。例:「メインのリポジトリは ~/work/app にある」'
      },
      toggleKind: 'クリックでルールとメモリを切り替え',
      source: {
        chat: 'チャットから',
        manual: '手動で追加'
      },
      includeInContext: 'コンテキストに含める',
      edit: '編集',
      save: '保存',
      cancel: 'キャンセル',
      delete: '削除',
      confirmDelete: 'このメモリ項目を削除しますか？',
      editHint: 'Enter で保存、Shift+Enter で改行、Esc でキャンセル',
      count: '{{total}} 件中 {{enabled}} 件が有効',
      empty: {
        title: 'メモリ項目はまだありません',
        description:
          '上の欄から追加するか、チャットでアシスタントに「覚えておいて」「ルールを追加して」と伝えてください。'
      },
      loading: 'メモリ項目を読み込み中…',
      error: 'メモリ項目を保存できませんでした: {{error}}',
      useMemoryItems: 'メモリ項目を使用',
      toggleOnTooltip: 'メモリ項目: オン（このチャットに {{count}} 件を追加）',
      toggleOffTooltip: 'メモリ項目: このチャットではオフ',
      noSession: 'チャットの開始後に利用できます',
      chip: {
        saved: 'メモリを保存しました',
        updated: 'メモリを更新しました',
        removed: 'メモリを削除しました',
        view: '表示',
        undo: '元に戻す',
        undone: '元に戻しました'
      }
    }
  }
}

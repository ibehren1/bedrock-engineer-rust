export const knowledgeBase = {
  en: {
    knowledgeBase: {
      tab: 'Knowledge Base',
      title: 'Local knowledge bases',
      description:
        'Documents and websites this agent can search. They are stored on this computer; an agent with a knowledge base gets the searchKnowledgeBase tool.',
      sharedUnavailable:
        'Shared agents cannot have local knowledge bases. Add one to your own copy of the agent instead.',
      empty: 'No knowledge bases yet.',
      create: 'New knowledge base',
      createTitle: 'New knowledge base',
      name: 'Name',
      namePlaceholder: 'e.g. Product docs',
      descriptionLabel: 'Description',
      descriptionPlaceholder: 'What it contains, so the model knows when to search it',
      createButton: 'Create',
      cancel: 'Cancel',
      save: 'Save',
      embeddingModel: 'Embedding model: {{model}}',
      embeddingFromSettings:
        'Set in Settings > Knowledge Base. It cannot be changed later without a rebuild.',
      rename: 'Rename',
      delete: 'Delete',
      deleteConfirm:
        'Delete the knowledge base "{{name}}"? Its indexed content is removed from this computer. The original files and websites are not touched.',
      deleted: 'Knowledge base deleted',
      rebuild: 'Rebuild',
      rebuildHint: 'Re-embed everything with the model now set in Settings ({{model}})',
      rebuildConfirm:
        'Re-embed every passage in "{{name}}" with {{model}}? This can take a while and, with Bedrock, costs embedding tokens.',
      rebuilding: 'Rebuilding…',
      modelMismatch: 'Built with {{model}}; Settings now use {{current}}.',
      addFiles: 'Add files',
      addWebsite: 'Add website',
      urlPlaceholder: 'https://docs.example.com/guide/',
      depth: 'Link depth',
      maxPages: 'Max pages',
      pathPrefixOnly: 'Stay under this path',
      pathPrefixOnlyHint: 'Only follow links below the start URL’s folder',
      crawlOptions: 'Link depth {{depth}} · up to {{pages}} pages',
      editCrawlOptions: 'Crawl settings',
      saveAndResync: 'Save and re-sync',
      stats: {
        passages: 'Passages',
        sourcesBreakdown: '{{documents}} documents · {{files}} files · {{websites}} websites',
        storage: 'Storage',
        reclaimable: '{{size}} can be reclaimed',
        compact: 'Compact',
        searches: 'Agent searches, {{days}} days',
        searchesPerDay: 'Agent searches per day, last {{days}} days',
        searchesOn: 'searches on {{day}}',
        lastUsed: 'Last used {{time}}',
        neverUsed: 'Not used by the agent yet',
        searchTime: 'Typical search time',
        searchTimeHint: 'Median time to embed the question and search, last 30 days',
        p95: '95% under {{time}}',
        noSearches: 'No searches yet',
        collapsed: '{{chunks}} passages · {{size}}'
      },
      health: {
        healthy: 'Healthy',
        attention: 'Needs attention',
        problem: 'Problem',
        suggestion: 'Suggestion',
        issue: {
          sourceErrors_one: '{{count}} source failed to sync. Re-sync or remove it.',
          sourceErrors_other: '{{count}} sources failed to sync. Re-sync or remove them.',
          staleWebsites_one: '{{count}} website was last synced over 30 days ago.',
          staleWebsites_other: '{{count}} websites were last synced over 30 days ago.',
          modelMismatch:
            'Built with a different embedding model than Settings now use. Rebuild to switch.',
          localModelMissing:
            'Built with the local model, which is not downloaded. Searches fail until it is.',
          reclaimable: '{{size}} of free space in the file. Optimize to reclaim it.',
          integrity: 'The integrity check failed. Rebuild or re-add the sources.',
          empty: 'No sources yet.'
        }
      },
      maintenance: {
        title: 'Maintenance',
        optimize: 'Optimize',
        optimizing: 'Optimizing…',
        optimizeHint:
          'Merges the keyword index, checks the file for errors and gives free space back. Runs on its own when a lot of space is free.',
        optimized: 'Optimized: {{before}} → {{after}}',
        integrityFailed: 'Integrity check failed: {{result}}',
        lastOptimized: 'Last optimized {{time}}',
        neverOptimized: 'Never optimized',
        lastJob: 'Last update {{time}}: {{chunks}} passages in {{duration}}'
      },
      builtinServer: {
        badge: 'Built-in',
        description:
          'Added automatically because this agent has local knowledge bases ({{names}}). Manage them on the Knowledge Base tab.',
        test: 'Test connection',
        connected: 'Connected: {{count}} tools available'
      },
      sources: 'Sources',
      noSources: 'Add files or a website to fill this knowledge base.',
      resync: 'Re-sync',
      remove: 'Remove',
      removeConfirm: 'Remove "{{name}}" and its indexed content from the knowledge base?',
      cancelJob: 'Stop',
      documents: '{{count}} documents',
      passages: '{{count}} passages',
      syncedAt: 'Synced {{time}}',
      status: {
        queued: 'Queued',
        processing: 'Processing',
        ready: 'Ready',
        error: 'Error',
        cancelled: 'Stopped'
      },
      phase: {
        queued: 'Waiting…',
        fetching: 'Crawling {{current}}/{{total}}',
        extracting: 'Reading…',
        embedding: 'Embedding {{current}}/{{total}}',
        indexing: 'Saving…',
        done: 'Done',
        error: 'Failed',
        cancelled: 'Stopped'
      },
      jobDone: '{{name}}: {{message}}',
      jobFailed: '{{name}} failed: {{message}}',
      searchTitle: 'Try a search',
      searchPlaceholder: 'Ask something the documents answer',
      search: 'Search',
      searching: 'Searching…',
      noResults: 'No matching passages.',
      score: 'score {{score}}',
      file: 'File',
      website: 'Website',
      settings: {
        embeddingTitle: 'Embedding model',
        embeddingDescription:
          'Turns text into vectors for search. New knowledge bases use this model; existing ones keep the model they were built with until you rebuild them.',
        provider: 'Provider',
        providerBedrock: 'Amazon Bedrock',
        providerLocal: 'On this computer',
        bedrockModel: 'Bedrock model',
        bedrockModelHint:
          'Must be enabled in the Bedrock console for your region. Bedrock bills per embedded token.',
        dimensions: 'Dimensions',
        localModel: 'Local model',
        localModelHint:
          'multilingual-e5-small runs on the CPU, works offline and supports about 100 languages. One-time download of {{size}}.',
        localInstalled: 'Downloaded ({{size}})',
        localNotInstalled: 'Not downloaded',
        localUnsupported: 'Not available in this build',
        download: 'Download model',
        downloading: 'Downloading {{percent}}%',
        cancelDownload: 'Cancel',
        deleteModel: 'Delete model',
        deleteModelConfirm:
          'Delete the local embedding model? Knowledge bases built with it cannot be searched until you download it again.',
        downloadDone: 'Local embedding model downloaded',
        retrievalTitle: 'Chunking and retrieval',
        retrievalDescription:
          'Chunk size and overlap apply to new knowledge bases. Results and weighting apply to every search.',
        chunkSize: 'Chunk size (tokens)',
        chunkOverlap: 'Chunk overlap (tokens)',
        topK: 'Results per search',
        hybridWeight: 'Semantic vs. keyword weight',
        hybridWeightHint:
          '1.0 ranks by meaning only, 0.0 by keyword matches only. Values in between combine both.',
        crawlerTitle: 'Website crawler',
        crawlerDescription:
          'Defaults for new website sources. The crawler stays on the start URL’s site.',
        maxDepth: 'Link depth',
        maxPages: 'Max pages',
        delayMs: 'Delay between requests (ms)',
        respectRobots: 'Respect robots.txt',
        useSitemap: 'Use sitemap.xml to find pages',
        overview: {
          knowledgeBases: 'Knowledge bases',
          acrossAgents_one: 'for {{count}} agent',
          acrossAgents_other: 'across {{count}} agents',
          documents: '{{n}} documents',
          typicalTime: 'Typical search {{time}}',
          optimizeAll: 'Optimize all',
          attention_one: '{{count}} knowledge base needs attention',
          attention_other: '{{count}} knowledge bases need attention',
          colKnowledgeBase: 'Knowledge base',
          colSearches: 'Agent searches ({{days}} d)',
          colSearchTime: 'Search time',
          colLastUsed: 'Last used',
          colHealth: 'Health',
          sources_one: '{{count}} source',
          sources_other: '{{count}} sources',
          p95Short: 'p95 {{time}}'
        },
        storageTitle: 'Overview',
        storageDescription: 'Knowledge bases on this computer: {{size}} in {{path}}',
        storageEmpty: 'No knowledge bases yet.',
        agent: 'Agent',
        orphan: 'Agent deleted',
        purgeOrphans: 'Delete orphaned knowledge bases',
        purgeConfirm: 'Delete {{count}} knowledge bases whose agent no longer exists?',
        purged: 'Deleted {{count}} knowledge bases',
        refresh: 'Refresh'
      }
    }
  },
  ja: {
    knowledgeBase: {
      tab: 'ナレッジベース',
      title: 'ローカルナレッジベース',
      description:
        'このエージェントが検索できるドキュメントとウェブサイトです。このコンピューターに保存され、ナレッジベースを持つエージェントには searchKnowledgeBase ツールが追加されます。',
      sharedUnavailable:
        '共有エージェントにはローカルナレッジベースを追加できません。自分用のエージェントのコピーに追加してください。',
      empty: 'ナレッジベースはまだありません。',
      create: '新しいナレッジベース',
      createTitle: '新しいナレッジベース',
      name: '名前',
      namePlaceholder: '例: 製品ドキュメント',
      descriptionLabel: '説明',
      descriptionPlaceholder: '内容の説明（モデルが検索するタイミングの判断に使います）',
      createButton: '作成',
      cancel: 'キャンセル',
      save: '保存',
      embeddingModel: '埋め込みモデル: {{model}}',
      embeddingFromSettings:
        '設定 > ナレッジベースで指定します。後から変更するには再構築が必要です。',
      rename: '名前を変更',
      delete: '削除',
      deleteConfirm:
        'ナレッジベース「{{name}}」を削除しますか？インデックス化された内容がこのコンピューターから削除されます。元のファイルやウェブサイトには影響しません。',
      deleted: 'ナレッジベースを削除しました',
      rebuild: '再構築',
      rebuildHint: '設定で現在指定しているモデル（{{model}}）ですべて埋め込み直します',
      rebuildConfirm:
        '「{{name}}」のすべてのパッセージを {{model}} で埋め込み直しますか？時間がかかることがあり、Bedrock の場合は埋め込みトークンの料金がかかります。',
      rebuilding: '再構築中…',
      modelMismatch: '{{model}} で構築されています。現在の設定は {{current}} です。',
      addFiles: 'ファイルを追加',
      addWebsite: 'ウェブサイトを追加',
      urlPlaceholder: 'https://docs.example.com/guide/',
      depth: 'リンクの深さ',
      maxPages: '最大ページ数',
      pathPrefixOnly: 'このパスの下に限定',
      pathPrefixOnlyHint: '開始 URL のフォルダより下のリンクだけをたどります',
      crawlOptions: 'リンクの深さ {{depth}} · 最大 {{pages}} ページ',
      editCrawlOptions: 'クロール設定',
      saveAndResync: '保存して再同期',
      stats: {
        passages: 'パッセージ',
        sourcesBreakdown:
          'ドキュメント {{documents}} · ファイル {{files}} · ウェブサイト {{websites}}',
        storage: 'ストレージ',
        reclaimable: '{{size}} を回収可能',
        compact: '最適化済み',
        searches: 'エージェントの検索（{{days}} 日間）',
        searchesPerDay: 'エージェントの 1 日あたりの検索数（過去 {{days}} 日）',
        searchesOn: '回（{{day}}）',
        lastUsed: '最終使用 {{time}}',
        neverUsed: 'エージェントはまだ使用していません',
        searchTime: '標準的な検索時間',
        searchTimeHint: '質問の埋め込みと検索にかかった時間の中央値（過去 30 日）',
        p95: '95% が {{time}} 以内',
        noSearches: '検索はまだありません',
        collapsed: 'パッセージ {{chunks}} 件 · {{size}}'
      },
      health: {
        healthy: '正常',
        attention: '要確認',
        problem: '問題あり',
        suggestion: '提案',
        issue: {
          sourceErrors_one:
            '{{count}} 件のソースの同期に失敗しました。再同期するか削除してください。',
          sourceErrors_other:
            '{{count}} 件のソースの同期に失敗しました。再同期するか削除してください。',
          staleWebsites_one: '{{count}} 件のウェブサイトが 30 日以上同期されていません。',
          staleWebsites_other: '{{count}} 件のウェブサイトが 30 日以上同期されていません。',
          modelMismatch:
            '現在の設定とは異なる埋め込みモデルで構築されています。切り替えるには再構築してください。',
          localModelMissing:
            'ローカルモデルで構築されていますが、モデルがダウンロードされていません。ダウンロードするまで検索できません。',
          reclaimable: 'ファイルに {{size}} の空き領域があります。最適化すると回収できます。',
          integrity: '整合性チェックに失敗しました。再構築するかソースを追加し直してください。',
          empty: 'ソースはまだありません。'
        }
      },
      maintenance: {
        title: 'メンテナンス',
        optimize: '最適化',
        optimizing: '最適化中…',
        optimizeHint:
          'キーワードインデックスを統合し、ファイルのエラーを確認して空き領域を回収します。空き領域が多い場合は自動的に実行されます。',
        optimized: '最適化しました: {{before}} → {{after}}',
        integrityFailed: '整合性チェックに失敗しました: {{result}}',
        lastOptimized: '最終最適化 {{time}}',
        neverOptimized: '未最適化',
        lastJob: '最終更新 {{time}}: パッセージ {{chunks}} 件、{{duration}}'
      },
      builtinServer: {
        badge: '組み込み',
        description:
          'このエージェントにはローカルナレッジベース（{{names}}）があるため自動的に追加されます。ナレッジベースタブで管理します。',
        test: '接続テスト',
        connected: '接続しました: {{count}} 個のツールが利用可能'
      },
      sources: 'ソース',
      noSources: 'ファイルまたはウェブサイトを追加してください。',
      resync: '再同期',
      remove: '削除',
      removeConfirm: '「{{name}}」とそのインデックス化された内容をナレッジベースから削除しますか？',
      cancelJob: '停止',
      documents: 'ドキュメント {{count}} 件',
      passages: 'パッセージ {{count}} 件',
      syncedAt: '{{time}} に同期',
      status: {
        queued: '待機中',
        processing: '処理中',
        ready: '準備完了',
        error: 'エラー',
        cancelled: '停止'
      },
      phase: {
        queued: '待機中…',
        fetching: 'クロール中 {{current}}/{{total}}',
        extracting: '読み込み中…',
        embedding: '埋め込み中 {{current}}/{{total}}',
        indexing: '保存中…',
        done: '完了',
        error: '失敗',
        cancelled: '停止'
      },
      jobDone: '{{name}}: {{message}}',
      jobFailed: '{{name}} の処理に失敗しました: {{message}}',
      searchTitle: '検索を試す',
      searchPlaceholder: 'ドキュメントに答えがある質問を入力',
      search: '検索',
      searching: '検索中…',
      noResults: '一致するパッセージはありません。',
      score: 'スコア {{score}}',
      file: 'ファイル',
      website: 'ウェブサイト',
      settings: {
        embeddingTitle: '埋め込みモデル',
        embeddingDescription:
          'テキストを検索用のベクトルに変換します。新しいナレッジベースはこのモデルを使い、既存のものは再構築するまで作成時のモデルのままです。',
        provider: 'プロバイダー',
        providerBedrock: 'Amazon Bedrock',
        providerLocal: 'このコンピューター',
        bedrockModel: 'Bedrock モデル',
        bedrockModelHint:
          'リージョンの Bedrock コンソールで有効化されている必要があります。Bedrock では埋め込んだトークン数に応じて課金されます。',
        dimensions: '次元数',
        localModel: 'ローカルモデル',
        localModelHint:
          'multilingual-e5-small は CPU で動作し、オフラインで使え、約 100 言語に対応します。初回のみ {{size}} をダウンロードします。',
        localInstalled: 'ダウンロード済み（{{size}}）',
        localNotInstalled: '未ダウンロード',
        localUnsupported: 'このビルドでは利用できません',
        download: 'モデルをダウンロード',
        downloading: 'ダウンロード中 {{percent}}%',
        cancelDownload: 'キャンセル',
        deleteModel: 'モデルを削除',
        deleteModelConfirm:
          'ローカル埋め込みモデルを削除しますか？このモデルで構築したナレッジベースは、再ダウンロードするまで検索できません。',
        downloadDone: 'ローカル埋め込みモデルをダウンロードしました',
        retrievalTitle: 'チャンク分割と検索',
        retrievalDescription:
          'チャンクサイズとオーバーラップは新しいナレッジベースに適用されます。結果数と重みはすべての検索に適用されます。',
        chunkSize: 'チャンクサイズ（トークン）',
        chunkOverlap: 'チャンクのオーバーラップ（トークン）',
        topK: '検索あたりの結果数',
        hybridWeight: '意味検索とキーワード検索の重み',
        hybridWeightHint:
          '1.0 は意味だけ、0.0 はキーワード一致だけで順位付けします。中間の値は両方を組み合わせます。',
        crawlerTitle: 'ウェブサイトのクローラー',
        crawlerDescription:
          '新しいウェブサイトソースの既定値です。クローラーは開始 URL のサイト内にとどまります。',
        maxDepth: 'リンクの深さ',
        maxPages: '最大ページ数',
        delayMs: 'リクエスト間隔（ミリ秒）',
        respectRobots: 'robots.txt に従う',
        useSitemap: 'sitemap.xml でページを探す',
        overview: {
          knowledgeBases: 'ナレッジベース',
          acrossAgents_one: 'エージェント {{count}} 件',
          acrossAgents_other: 'エージェント {{count}} 件',
          documents: 'ドキュメント {{n}} 件',
          typicalTime: '標準的な検索 {{time}}',
          optimizeAll: 'すべて最適化',
          attention_one: '{{count}} 件のナレッジベースに確認が必要です',
          attention_other: '{{count}} 件のナレッジベースに確認が必要です',
          colKnowledgeBase: 'ナレッジベース',
          colSearches: 'エージェントの検索（{{days}} 日）',
          colSearchTime: '検索時間',
          colLastUsed: '最終使用',
          colHealth: '状態',
          sources_one: 'ソース {{count}} 件',
          sources_other: 'ソース {{count}} 件',
          p95Short: 'p95 {{time}}'
        },
        storageTitle: '概要',
        storageDescription: 'このコンピューター上のナレッジベース: {{path}} に {{size}}',
        storageEmpty: 'ナレッジベースはまだありません。',
        agent: 'エージェント',
        orphan: 'エージェント削除済み',
        purgeOrphans: '孤立したナレッジベースを削除',
        purgeConfirm: 'エージェントが存在しないナレッジベース {{count}} 件を削除しますか？',
        purged: 'ナレッジベース {{count}} 件を削除しました',
        refresh: '更新'
      }
    }
  }
}

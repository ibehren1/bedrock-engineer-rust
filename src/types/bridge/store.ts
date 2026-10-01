// Shape of the settings file (config.json) as the renderer sees it through `window.store`.
// The Rust `store` crate (src-tauri/crates/store) owns the file, its defaults and migrations.
import { LLM, InferenceParameters, ThinkingMode } from '../llm'
import {
  AgentChatConfig,
  KnowledgeBase,
  SendMsgKey,
  ToolState,
  OrganizationConfig,
  CustomAgent
} from '../agent-chat'
import { BedrockAgent } from '../agent'
import { AWSCredentials } from '../aws'
import { CodeInterpreterContainerConfig, DockerSandboxConfig } from '../sandbox'

export type StoreScheme = {
  /** アプリケーションのユーザーデータ保存先パス（config.json のあるディレクトリ） */
  userDataPath?: string

  /** 現在選択されているプロジェクト（作業ディレクトリ）のパス */
  projectPath?: string

  /** Plan/Act モードの設定 (true: Plan, false: Act) */
  planMode?: boolean

  /** アプリの外観テーマ（明るい順に 'light' | 'newspaper' | 'dim' | 'charcoal' | 'dark'、および OS 追従の 'system'）。デフォルトは 'dim' */
  appTheme?: 'light' | 'newspaper' | 'dim' | 'charcoal' | 'dark' | 'system'

  /** UI テキストの書体。デフォルトは 'inter' */
  appFontSans?: 'inter' | 'geist' | 'system'

  /** コード・ID・パスなど等幅テキストの書体。デフォルトは 'jetbrains' */
  appFontMono?: 'jetbrains' | 'geist-mono' | 'system'

  /** 現在選択されている言語モデル (LLM) の設定 */
  llm?: LLM

  /** 軽微な処理（タイトル生成など）に使用するモデルの設定 */
  lightProcessingModel?: LLM | null

  /** 言語モデルの推論パラメータ（温度、最大トークン数など） */
  inferenceParams: InferenceParameters

  /** 思考モードの設定（Claude 3.7 Sonnet用） */
  thinkingMode?: ThinkingMode

  /** インターリーブ思考の設定（思考モードの拡張機能） */
  interleaveThinking?: boolean

  /** 画像認識ツールの設定 */
  recognizeImageTool?: {
    /** 使用するモデルID */
    modelId: string
  }

  /** 画像生成ツールの設定 */
  generateImageTool?: {
    /** 使用するモデルID */
    modelId: string
  }

  /** 動画生成ツールの設定 */
  generateVideoTool?: {
    /** S3出力先URI */
    s3Uri: string
  }

  /** コードインタープリタツールの設定 */
  codeInterpreterTool?: CodeInterpreterContainerConfig

  /** Docker サンドボックスツールの設定（チャットごとのコンテナのリソース上限） */
  dockerSandboxTool?: DockerSandboxConfig

  /** アプリケーションの表示言語設定（日本語または英語） */
  language: 'ja' | 'en'

  /** エージェントチャットの設定（無視するファイル一覧、コンテキスト長など） */
  agentChatConfig: AgentChatConfig

  /** 使用可能なツールの状態と設定（有効/無効、設定情報） */
  tools: ToolState[]

  /** ウェブサイトジェネレーター機能の設定 */
  websiteGenerator?: {
    /** 使用する知識ベース一覧 */
    knowledgeBases?: KnowledgeBase[]
    /** 知識ベース機能を有効にするかどうか */
    enableKnowledgeBase?: boolean
    /** 検索機能を有効にするかどうか */
    enableSearch?: boolean
  }

  /** Tavily検索APIの設定 */
  tavilySearch: {
    /** Tavily検索APIのAPIキー */
    apikey: string
  }

  /** 高度な設定オプション */
  advancedSetting: {
    /** キーボードショートカット設定 */
    keybinding: {
      /** メッセージ送信キーの設定（EnterまたはCmd+Enter） */
      sendMsgKey: SendMsgKey
    }
  }

  /** AWS認証情報とリージョン設定 */
  aws: AWSCredentials

  /** ユーザーが作成したカスタムエージェントの一覧 */
  customAgents: CustomAgent[]

  /** 現在選択されているエージェントのID */
  selectedAgentId: string

  /** 使用可能な知識ベース一覧 */
  knowledgeBases: KnowledgeBase[]

  /** コマンド実行の設定（シェル設定） */
  shell: string

  /** 通知機能の有効/無効設定 */
  notification?: boolean

  /** サイドバーで非表示にするナビゲーション項目のhref一覧 */
  sidebarHiddenItems?: string[]

  /** ユーザーが削除したデフォルトエージェントのID一覧（再シードを防ぐ） */
  hiddenDefaultAgentIds?: string[]

  /** ユーザーがドラッグ＆ドロップで並べ替えたエージェントのID順 */
  agentOrder?: string[]

  /** Amazon Bedrock特有の設定 */
  bedrockSettings?: {
    /** リージョンフェイルオーバー機能の有効/無効 */
    enableRegionFailover: boolean
    /** フェイルオーバー時に使用可能なリージョン一覧 */
    availableFailoverRegions: string[]
    /** アプリケーション推論プロファイル機能の有効/無効 */
    enableInferenceProfiles: boolean
    /** チャットのモデル選択に表示するモデルID一覧（空の場合は全て表示） */
    visibleModelIds?: string[]
  }

  /** ガードレール設定 */
  guardrailSettings?: {
    /** ガードレールを有効にするかどうか */
    enabled: boolean
    /** ガードレールID */
    guardrailIdentifier: string
    /** ガードレールバージョン */
    guardrailVersion: string
    /** ガードレールのトレース設定 */
    trace: 'enabled' | 'disabled'
  }

  /** 使用可能なAmazon Bedrockエージェントの一覧 */
  bedrockAgents?: BedrockAgent[]

  /** YAML形式から読み込まれた共有エージェントの一覧 */
  sharedAgents?: CustomAgent[]

  /** Nova Sonic音声チャットで使用する音声ID */
  selectedVoiceId?: string

  /** バックグラウンドエージェントのスケジュールタスク */
  backgroundAgentScheduledTasks?: any[]

  /** 組織設定の一覧 */
  organizations?: OrganizationConfig[]

  /** エージェントリストの表示モード（カードまたはテーブル） */
  agentListViewMode?: 'card' | 'table'

  /** チャットのユーザーアバターに表示する絵文字（空の場合はデフォルトアイコン） */
  userEmoji?: string

  /** チャットのユーザーラベルに表示する名前（空の場合はデフォルトの "user" ラベル） */
  userName?: string
}

type Key = keyof StoreScheme

/** `window.store` (installed by src/renderer/src/lib/tauriBridge.ts). */
export type ConfigStore = {
  get<T extends Key>(key: T): StoreScheme[T]
  set<T extends Key>(key: T, value: StoreScheme[T]): void
}

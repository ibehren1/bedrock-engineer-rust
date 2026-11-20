/**
 * Strings on the AWS settings tab: the sign-in method cards (`awsAuth.*`), the region select
 * and a few general settings strings that previously had no key.
 */
export const awsSettings = {
  en: {
    'AWS Authentication': 'AWS Authentication',
    'This application requires specific IAM permissions to access Amazon Bedrock.':
      'This application requires specific IAM permissions to access Amazon Bedrock.',
    'View required IAM policies': 'View required IAM policies',
    'AWS Region': 'AWS Region',
    'AWS Access Key ID': 'AWS Access Key ID',
    'AWS Secret Access Key': 'AWS Secret Access Key',
    'AWS Session Token (optional)': 'AWS Session Token (optional)',
    'Select a region': 'Select a region',
    'Bedrock Supported Regions': 'Bedrock Supported Regions',
    'Other Regions': 'Other Regions',
    Language: 'Language',
    'Select Language': 'Select Language',
    'Request Timeout (minutes)': 'Request Timeout (minutes)',
    awsAuth: {
      method: 'Sign-in method',
      accessKey: {
        title: 'Access key / secret key',
        description: 'Long-term or temporary IAM credentials entered here.'
      },
      profile: {
        title: 'AWS profile',
        description: 'A named profile from the AWS CLI configuration, including SSO profiles.',
        hint: 'Reads ~/.aws/config and ~/.aws/credentials',
        refresh: 'Refresh profile list',
        loading: 'Loading profiles…',
        loadError: 'Could not read the AWS profiles: {{error}}',
        notFound: '{{name}} (not found)',
        select: 'Select a profile',
        none: 'No AWS profiles found. Create one with either command below, then press refresh.'
      },
      apiKey: {
        title: 'Bedrock API key',
        description: 'An Amazon Bedrock API key, for Bedrock only.',
        label: 'Bedrock API key',
        hint: 'Create one in the Amazon Bedrock console, on the API keys page.',
        unavailableTitle: 'Not available with a Bedrock API key:',
        unavailable: {
          agents: 'Bedrock Agents',
          knowledgeBases: 'Bedrock Knowledge Bases (Amazon Bedrock retrieve)',
          flows: 'Bedrock Flows',
          translate: 'Translation (Amazon Translate)',
          video: 'Video generation',
          agentFiles: 'Agent file uploads to S3'
        },
        unavailableHint: 'Use access keys or an AWS profile for these.'
      }
    }
  },
  ja: {
    'AWS Authentication': 'AWS 認証',
    'This application requires specific IAM permissions to access Amazon Bedrock.':
      'このアプリケーションが Amazon Bedrock にアクセスするには、特定の IAM 権限が必要です。',
    'View required IAM policies': '必要な IAM ポリシーを表示',
    'AWS Region': 'AWS リージョン',
    'AWS Access Key ID': 'AWS アクセスキー ID',
    'AWS Secret Access Key': 'AWS シークレットアクセスキー',
    'AWS Session Token (optional)': 'AWS セッショントークン（任意）',
    'Select a region': 'リージョンを選択',
    'Bedrock Supported Regions': 'Bedrock 対応リージョン',
    'Other Regions': 'その他のリージョン',
    Language: '言語',
    'Select Language': '言語を選択',
    'Request Timeout (minutes)': 'リクエストのタイムアウト（分）',
    awsAuth: {
      method: 'サインイン方法',
      accessKey: {
        title: 'アクセスキー / シークレットキー',
        description: 'ここに入力する長期または一時的な IAM 認証情報。'
      },
      profile: {
        title: 'AWS プロファイル',
        description: 'AWS CLI 設定の名前付きプロファイル（SSO プロファイルを含む）。',
        hint: '~/.aws/config と ~/.aws/credentials を読み取ります',
        refresh: 'プロファイル一覧を更新',
        loading: 'プロファイルを読み込み中…',
        loadError: 'AWS プロファイルを読み取れませんでした: {{error}}',
        notFound: '{{name}}（見つかりません）',
        select: 'プロファイルを選択',
        none: 'AWS プロファイルが見つかりません。次のいずれかのコマンドで作成してから、更新を押してください。'
      },
      apiKey: {
        title: 'Bedrock API キー',
        description: 'Amazon Bedrock 専用の API キー。',
        label: 'Bedrock API キー',
        hint: 'Amazon Bedrock コンソールの API キーページで作成します。',
        unavailableTitle: 'Bedrock API キーでは使用できない機能:',
        unavailable: {
          agents: 'Bedrock エージェント',
          knowledgeBases: 'Bedrock ナレッジベース（Amazon Bedrock retrieve）',
          flows: 'Bedrock フロー',
          translate: '翻訳（Amazon Translate）',
          video: '動画生成',
          agentFiles: 'エージェントファイルの S3 へのアップロード'
        },
        unavailableHint: 'これらにはアクセスキーまたは AWS プロファイルを使用してください。'
      }
    }
  }
}

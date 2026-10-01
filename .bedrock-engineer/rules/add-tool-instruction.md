# Bedrock Engineer ツール実装ガイドライン

## 概要

Bedrock Engineerは拡張可能なAIエージェントプラットフォームであり、様々なツールを追加することで機能を拡張できます。このガイドラインは、新しいツールを実装する際の標準的なアプローチを提供します。

## ツール実装の基本構造

ツールは Rust バックエンド（`src-tauri/`）で実行され、画面は React レンダラー（`src/renderer/`）にあります。新しいツールを実装する際は、以下の主要コンポーネントを開発する必要があります：

1. **Rust のツール実装**: `tools` クレートで `Tool` トレイトを実装（主要ロジック・入力検証・ツール仕様）
2. **レジストリへの登録**: ツールをレジストリに追加し、AIエージェントから名前で呼び出せるようにする
3. **型定義**: レンダラー側のツール入出力データ構造
4. **UI設定コンポーネント**: ツール設定のためのユーザーインターフェース
5. **テスト**: ツール機能の検証（Rust のテスト）

## 実装ステップ

### 1. ツールの実装（Rust）

```rust
// src-tauri/crates/tools/src/your_tool.rs
use crate::base::Tool;
use crate::context::ToolContext;
use crate::error::Result;
use crate::types::{ToolCategory, ToolOutput, ToolSpec};
use async_trait::async_trait;
use serde_json::{json, Value};

const NAME: &str = "yourTool";
const DESCRIPTION: &str = "あなたのツールの説明";

pub struct YourTool;

#[async_trait]
impl Tool for YourTool {
    fn name(&self) -> &str {
        NAME
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn spec(&self) -> Option<ToolSpec> {
        Some(ToolSpec::new(
            NAME,
            DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "requiredParam": { "type": "string", "description": "必須パラメータの説明" }
                },
                "required": ["requiredParam"]
            }),
        ))
    }

    // 入力検証：エラーメッセージの一覧を返す（空なら有効）
    fn validate_input(&self, input: &Value) -> Vec<String> {
        match input.get("requiredParam") {
            Some(Value::String(_)) => Vec::new(),
            _ => vec!["Missing required parameter: requiredParam".to_string()],
        }
    }

    async fn execute_internal(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        // ツールの主要ロジックを実装（外部APIの呼び出しやデータ処理など）
        Ok(ToolOutput::Json(json!({
            "success": true,
            "name": NAME,
            "message": "Your tool executed successfully",
            "result": { "echo": input["requiredParam"] }
        })))
    }
}
```

AWS を呼び出すツールは `bedrock` クレートのクライアント（`src-tauri/crates/bedrock`）を使い、既存の Bedrock ツール（`src-tauri/crates/tools/src/bedrock_tools/`）と同じく依存をコンストラクタで受け取ります。

### 2. ツールモジュールの公開

```rust
// src-tauri/crates/tools/src/lib.rs
pub mod your_tool;
```

### 3. レジストリへの登録

```rust
// src-tauri/crates/tools/src/deps.rs の all_tools()
all.push(Arc::new(your_tool::YourTool));
```

`all_tools()` がアプリのツールレジストリの中身です。ツールはレンダラーから `tools_execute` コマンド経由で、バックグラウンドエージェントやサブエージェントからは Rust のエージェントループ（`src-tauri/crates/agents`）から名前で呼び出されます。

### 4. レンダラーから見えるツール一覧

ツール仕様（`spec()`）は起動時に `tools_get_tool_specs` でレンダラーに渡され、`window.api.tools.getToolSpecs()` から取得できます。特別な IPC の追加は不要です。

### 5. 型定義の追加

```typescript
// src/types/tools.ts
export type YourToolInput = {
  type: 'yourTool'
  requiredParam: string
  optionalParam?: number
}

// ToolInput型の拡張
export type ToolInput = ExistingToolInput | YourToolInput
// その他のツール入力型

// ToolInputTypeMap の拡張
export type ToolInputTypeMap = {
  // 既存のマッピング
  yourTool: YourToolInput
}

// ツール定義の追加
export const TOOLS: Tool[] = [
  // 既存のツール定義
  {
    toolSpec: {
      name: 'yourTool',
      description: 'あなたのツールの説明',
      inputSchema: {
        json: {
          type: 'object',
          properties: {
            requiredParam: {
              type: 'string',
              description: '必須パラメータの説明'
            },
            optionalParam: {
              type: 'number',
              description: 'オプションパラメータの説明'
            }
          },
          required: ['requiredParam']
        }
      }
    }
  }
]
```

### 6. エージェント型定義の拡張

```typescript
// src/types/agent-chat.ts
export type YourToolConfig = {
  configParam1: string
  configParam2: number
}

export type CustomAgent = Agent & {
  // 既存のプロパティ
  yourToolConfigs?: YourToolConfig[] // エージェント固有のツール設定
}
```

### 7. UI設定コンポーネントの実装

```typescript
// src/renderer/src/pages/ChatPage/modals/useToolSettingModal/YourToolSettingForm.tsx
import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { YourToolConfig } from '@/types/agent-chat'

export interface YourToolSettingFormProps {
  configs: YourToolConfig[]
  setConfigs: (configs: YourToolConfig[]) => void
}

export const YourToolSettingForm: React.FC<YourToolSettingFormProps> = ({ configs, setConfigs }) => {
  const { t } = useTranslation()
  // フォーム状態の管理

  return (
    <div className="p-4">
      <h3 className="text-lg font-medium mb-4">{t('Your Tool Settings')}</h3>

      {/* 設定フォームのUI実装 */}
    </div>
  )
}
```

### 8. 設定モーダルへの統合

```typescript
// src/renderer/src/pages/ChatPage/modals/useToolSettingModal/index.tsx
import { YourToolSettingForm } from './YourToolSettingForm'

// TOOLS_WITH_SETTINGS に追加
const TOOLS_WITH_SETTINGS = [
  // 既存のツール
  'yourTool'
]

// レンダリング部分に追加
{selectedTool === 'yourTool' && selectedAgentId && (
  <YourToolSettingForm
    configs={getAgentYourToolConfigs(selectedAgentId)}
    setConfigs={(configs) => updateAgentYourToolConfigs(selectedAgentId, configs)}
  />
)}
```

### 9. 国際化対応

```typescript
// src/renderer/src/i18n/locales/chat/tools.ts
export const tools = {
  en: {
    'tool names': {
      // 既存のツール名
      yourTool: 'Your Tool Name'
    }
  },
  ja: {
    'tool names': {
      // 既存のツール名
      yourTool: 'あなたのツール名'
    }
  }
}

// src/renderer/src/i18n/locales/settings/agentToolsSettings.ts
export const agentToolsSettings = {
  en: {
    descriptions: {
      // 既存のツール説明
      yourTool: 'Description of your tool'
    }
  },
  ja: {
    descriptions: {
      // 既存のツール説明
      yourTool: 'あなたのツールの説明'
    }
  }
}
```

### 10. アイコンの追加

```typescript
// src/renderer/src/pages/ChatPage/components/Tool/ToolIcons.tsx
import { YourToolIcon } from 'react-icons/fa'

const standardToolIcons = {
  // 既存のアイコン
  yourTool: <YourToolIcon className="text-blue-600 size-6" />
}
```

### 11. デフォルトツールセットへの追加

```typescript
// src/renderer/src/constants/defaultToolSets.ts
const DEFAULT_TOOL_NAMES: Record<string, string[]> = {
  general: [
    // 既存のツール
    'yourTool'
  ]
  // その他のカテゴリ
}
```

### 12. ツールカテゴリへの追加

```typescript
// src/renderer/src/pages/ChatPage/components/AgentForm/ToolsSection/utils/toolCategories.ts
export const TOOL_CATEGORIES: ToolCategory[] = [
  // 既存のカテゴリ
  {
    id: 'your-category',
    name: 'Your Category',
    description: 'Description of your category',
    tools: [
      // 既存のツール
      'yourTool'
    ]
  }
]
```

### 13. テストの実装

```rust
// src-tauri/crates/tools/src/your_tool.rs の末尾
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_missing_required_param() {
        assert!(!YourTool.validate_input(&json!({})).is_empty());
    }

    #[test]
    fn accepts_a_valid_input() {
        assert!(YourTool.validate_input(&json!({ "requiredParam": "x" })).is_empty());
    }
}
```

`src-tauri/` で `cargo test --workspace` と `cargo clippy --workspace --all-targets` を実行します。実際の AWS 認証情報や Docker が必要なテストには `#[ignore]` を付けます（`cargo test --workspace -- --ignored` で実行）。

## ベストプラクティス

1. **型安全性**: Rust と TypeScript の型システムを活用し、入出力の型を明確に定義する
2. **エラーハンドリング**: すべての例外を適切にキャッチし、意味のあるエラーメッセージを提供する
3. **ロギング**: 重要な操作とエラーを適切にログに記録する
4. **テスト**: 単体テストと統合テストを実装し、ツールの動作を検証する
5. **国際化**: すべてのUIテキストを国際化対応する
6. **設定の永続化**: エージェント固有の設定を適切に保存・読み込みする
7. **UIの一貫性**: 既存のUIコンポーネントとスタイルを活用し、一貫したユーザー体験を提供する
8. **ドキュメント**: コードにコメントを追加し、複雑なロジックを説明する

## セキュリティ考慮事項

1. **入力検証**: すべてのユーザー入力を検証し、不正な値を拒否する
2. **認証情報の保護**: APIキーなどの認証情報を安全に保存・管理する
3. **権限管理**: ツールが適切な権限でのみ実行されるようにする

## パフォーマンス最適化

1. **非同期処理**: 長時間実行される操作は非同期で実行し、UIをブロックしない
2. **キャッシュ**: 頻繁に使用されるデータをキャッシュし、パフォーマンスを向上させる
3. **バッチ処理**: 複数の操作をバッチ処理し、APIコールを最小限に抑える

## デバッグとトラブルシューティング

1. **詳細なログ**: 開発中は詳細なログを有効にし、問題の診断を容易にする
2. **エラーコード**: 一般的なエラーに対して明確なエラーコードを定義する
3. **テスト環境**: 本番環境に影響を与えずにツールをテストできる環境を用意する

このガイドラインに従うことで、Bedrock Engineerに一貫した方法で新しいツールを追加し、プラットフォームの機能を拡張することができます。

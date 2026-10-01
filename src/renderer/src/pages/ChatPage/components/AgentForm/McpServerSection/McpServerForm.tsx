import React from 'react'
import { useTranslation } from 'react-i18next'
import { McpServerConfig } from '@/types/agent-chat'
import { parseServerConfigJson, generateSampleJson } from './utils/mcpServerUtils'
import { preventModalClose } from './utils/eventUtils'
import toast from 'react-hot-toast'
import { Button } from '@renderer/components/ui'

interface McpServerFormProps {
  mcpServers: McpServerConfig[]
  onChange: (servers: McpServerConfig[]) => void
  jsonInput: string
  setJsonInput: (value: string) => void
  jsonError: string | null
  setJsonError: (error: string | null) => void
  editMode: string | null
  setEditMode: (mode: string | null) => void
  autoTestOnAdd: boolean
  testServerConnection: (serverName: string, serverList?: McpServerConfig[]) => Promise<void>
}

/**
 * MCPサーバー追加/編集フォームコンポーネント
 */
export const McpServerForm: React.FC<McpServerFormProps> = ({
  mcpServers,
  onChange,
  jsonInput,
  setJsonInput,
  jsonError,
  setJsonError,
  editMode,
  setEditMode,
  autoTestOnAdd,
  testServerConnection
}) => {
  const { t } = useTranslation()

  // 追加ボタンクリック時に入力されたJSONを解析して追加
  const handleAddServer = async () => {
    const result = parseServerConfigJson(jsonInput, mcpServers)

    if (!result.success) {
      setJsonError(t(result.error || 'Invalid JSON format.'))
      return
    }

    // サーバー設定を追加
    const updatedServers = [...mcpServers, ...(result.servers || [])]
    onChange(updatedServers)

    // 入力欄をクリア
    setJsonInput('')
    setJsonError(null)

    // 成功メッセージ
    const count = result.servers?.length || 0
    toast.success(`${count} MCP server(s) added successfully`, {
      duration: 3000
    })

    // 自動接続テストが有効で、新しいサーバーが追加された場合
    if (autoTestOnAdd && result.servers && result.servers.length > 0) {
      // サーバーリストを安全に参照するためローカル変数に格納
      const serversToTest = [...result.servers]

      // 少し待ってからテストを実行（UIの更新が完了するのを待つ）
      setTimeout(() => {
        // 単一サーバーの場合は従来通り
        if (serversToTest.length === 1) {
          testServerConnection(serversToTest[0].name, updatedServers)
        }
        // 複数サーバーの場合は追加された全サーバーをテスト
        else {
          console.log(`Testing all ${serversToTest.length} newly added MCP servers...`)
          // 追加されたサーバーのみループでテスト
          for (const server of serversToTest) {
            testServerConnection(server.name, updatedServers)
          }
        }
      }, 500)
    }
  }

  // 編集内容保存
  const handleSaveEdit = () => {
    // 編集中のサーバーを除外したサーバーリストを作成（重複チェック用）
    const serversExcludingEditTarget = mcpServers.filter((server) => server.name !== editMode)

    // parseServerConfigJson を使用して入力されたJSONを解析
    const result = parseServerConfigJson(jsonInput, serversExcludingEditTarget)

    if (!result.success) {
      setJsonError(t(result.error || 'Invalid JSON format.'))
      return
    }

    if (!result.servers || result.servers.length === 0) {
      setJsonError(t('No valid server configurations found'))
      return
    }

    // 複数サーバーが含まれている場合の処理
    if (result.servers.length > 1) {
      // 編集対象サーバーを削除し、新しいサーバー設定を追加
      const updatedServers = [...serversExcludingEditTarget, ...result.servers]
      onChange(updatedServers)

      // 成功メッセージ
      toast.success(t('Multiple servers updated successfully'), {
        duration: 3000
      })
    } else {
      // 単一サーバーの場合は従来通りの更新
      const updatedServer = result.servers[0]
      const updatedServers = mcpServers.map((server) =>
        server.name === editMode ? updatedServer : server
      )
      onChange(updatedServers)

      // 成功メッセージ
      toast.success(t('Server updated successfully'), {
        duration: 3000
      })
    }

    // 編集モード終了
    setEditMode(null)
    setJsonInput('')
    setJsonError(null)
  }

  // 編集キャンセル
  const handleCancelEdit = () => {
    setEditMode(null)
    setJsonInput('')
    setJsonError(null)
  }

  return (
    <div
      className="flex flex-col gap-2 mt-4 border border-subtle p-2.5 rounded-control"
      onClick={preventModalClose}
    >
      <h4 className="font-medium text-sm mb-2 text-ink">
        {editMode ? t('Edit MCP Server') : t('Add New MCP Server')}
      </h4>

      <div className="mt-2">
        <label htmlFor="jsonInput" className="block text-sm font-medium text-ink">
          {t('Server Configuration (JSON)')}
        </label>

        <button
          type="button"
          className="text-xs text-accent hover:text-accent"
          onClick={(e) => {
            e.preventDefault()
            e.stopPropagation()
            setJsonInput(generateSampleJson())
          }}
        >
          {t('Set example mcp server')}
        </button>
        <textarea
          id="jsonInput"
          value={jsonInput}
          onChange={(e) => {
            setJsonInput(e.target.value)
            if (jsonError) setJsonError(null)
          }}
          onClick={preventModalClose}
          className="mt-1 block w-full h-64 px-3 py-2 bg-surface text-ink border border-strong rounded-control shadow-sm focus:outline-none focus:ring-accent focus:border-accent sm:text-sm font-mono"
          placeholder={`{
  "mcpServers": {
    "my-mcp-server": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/directory"],
      "env": { "VAR": "value" }
    },
    "api-server": {
      "url": "https://api.example.com/mcp",
      "headers": {
        "Authorization": "Bearer your-token-here",
        "X-API-Key": "your-api-key"
      }
    }
  }
}`}
        />
        {jsonError && <p className="text-xs text-danger mt-1 whitespace-pre-line">{jsonError}</p>}
        <p className="text-xs text-ink-muted mt-1">
          {t(
            'Use claude_desktop_config.json format with mcpServers object containing server configurations.'
          )}
        </p>
      </div>

      <div className="flex gap-2">
        {editMode ? (
          <>
            <button
              type="button"
              onClick={(e) => {
                e.preventDefault()
                e.stopPropagation()
                handleCancelEdit()
              }}
              className="px-3 py-1.5 text-sm text-ink-muted border border-strong rounded-control hover:bg-raised"
            >
              {t('Cancel')}
            </button>
            <Button
              type="button"
              onClick={(e) => {
                e.preventDefault()
                e.stopPropagation()
                handleSaveEdit()
              }}
              variant="primary"
            >
              {t('Update Server')}
            </Button>
          </>
        ) : (
          <Button
            type="button"
            onClick={(e) => {
              e.preventDefault()
              e.stopPropagation()
              handleAddServer()
            }}
            variant="primary"
            className="mt-2 w-fit"
          >
            {t('Add Server')}
          </Button>
        )}
      </div>
    </div>
  )
}

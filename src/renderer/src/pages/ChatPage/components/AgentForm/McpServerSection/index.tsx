import React from 'react'
import { useTranslation } from 'react-i18next'
import { McpServerConfig } from '@/types/agent-chat'
import { McpServerList } from './McpServerList'
import { McpServerForm } from './McpServerForm'
import { useMcpServerState } from './hooks/useMcpServerState'
import { preventModalClose } from './utils/eventUtils'
import { generateEditJson } from './utils/mcpServerUtils'
import { McpMarketPanel } from './McpMarketPanel'
import { AgentMarketContext } from './mcpMarket'

interface McpServerSectionProps {
  mcpServers: McpServerConfig[]
  onChange: (mcpServers: McpServerConfig[]) => void
  /** The agent being edited, used to tailor MCP Market links and suggestions */
  agentContext?: AgentMarketContext
}

/**
 * MCPサーバー設定セクションのメインコンポーネント
 */
export const McpServerSection: React.FC<McpServerSectionProps> = ({
  mcpServers,
  onChange,
  agentContext
}) => {
  const { t } = useTranslation()

  // カスタムフックを使用して状態を管理
  const {
    // フォーム関連
    jsonInput,
    setJsonInput,
    jsonError,
    setJsonError,
    editMode,
    setEditMode,

    // 接続テスト関連
    testingConnection,
    testingAll,
    connectionResults,
    testServerConnection,
    testAllConnections,
    clearConnectionResults,

    // その他
    autoTestOnAdd,
    handleDelete
  } = useMcpServerState(mcpServers, onChange)

  // 編集モードに切り替え
  const handleEdit = (serverName: string) => {
    const serverToEdit = mcpServers.find((server) => server.name === serverName)
    if (serverToEdit) {
      setJsonInput(generateEditJson(serverToEdit))
      setEditMode(serverName)
    }
  }

  /**
   * Load a suggested server into the JSON editor. It is deliberately *not*
   * added or connected here: suggestions come from a model, so the user reviews
   * the command before anything runs.
   */
  const handleUseSuggestion = (json: string) => {
    setEditMode(null)
    setJsonError(null)
    setJsonInput(json)
  }

  return (
    <div className="space-y-2" onClick={preventModalClose}>
      <h3 className="text-heading font-semibold text-ink mb-4">{t('MCP Server Settings')}</h3>

      <div className="bg-raised p-3 rounded-control mb-4 border border-subtle">
        <p className="text-sm text-ink">
          {t('Configure MCP servers for this agent to use MCP tools.')}
        </p>
        <p className="text-xs text-ink-muted mt-1">
          {t(
            'Register MCP servers first, then you can enable MCP tools in the Available Tools tab.'
          )}
        </p>
      </div>

      {/* MCP Market: links and model-suggested servers for this agent */}
      <McpMarketPanel
        agentContext={agentContext || {}}
        existingServerNames={mcpServers.map((server) => server.name)}
        onUseSuggestion={handleUseSuggestion}
      />

      {/* サーバーリスト */}
      <McpServerList
        mcpServers={mcpServers}
        onEdit={handleEdit}
        onDelete={handleDelete}
        testingConnection={testingConnection}
        testingAll={testingAll}
        connectionResults={connectionResults}
        testServerConnection={testServerConnection}
        testAllConnections={testAllConnections}
        clearConnectionResults={clearConnectionResults}
      />

      {/* サーバー追加/編集フォーム */}
      <McpServerForm
        mcpServers={mcpServers}
        onChange={onChange}
        jsonInput={jsonInput}
        setJsonInput={setJsonInput}
        jsonError={jsonError}
        setJsonError={setJsonError}
        editMode={editMode}
        setEditMode={setEditMode}
        autoTestOnAdd={autoTestOnAdd}
        testServerConnection={testServerConnection}
      />
    </div>
  )
}

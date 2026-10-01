import React from 'react'
import { useTranslation } from 'react-i18next'
import { McpServerConfig } from '@/types/agent-chat'
import { FiZap } from 'react-icons/fi'
import { ServerListItem } from './ServerListItem'
import { ConnectionTestResults } from './ConnectionTestResults'
import { preventModalClose } from './utils/eventUtils'

interface McpServerListProps {
  mcpServers: McpServerConfig[]
  onEdit: (serverName: string) => void
  onDelete: (serverName: string) => void
  testingConnection: string | null
  testingAll: boolean
  connectionResults: Record<string, any>
  testServerConnection: (serverName: string) => Promise<void>
  testAllConnections: () => Promise<void>
  clearConnectionResults: () => void
}

/**
 * MCPサーバーリストを表示するコンポーネント
 */
export const McpServerList: React.FC<McpServerListProps> = ({
  mcpServers,
  onEdit,
  onDelete,
  testingConnection,
  testingAll,
  connectionResults,
  testServerConnection,
  testAllConnections,
  clearConnectionResults
}) => {
  const { t } = useTranslation()

  if (mcpServers.length === 0) {
    return (
      <div className="text-center p-2.5 border border-subtle rounded-control">
        <p className="text-ink-muted">{t('No MCP servers configured yet')}</p>
      </div>
    )
  }

  return (
    <div className="space-y-2">
      <div className="flex justify-between items-center">
        <h4 className="font-medium text-sm text-ink">{t('Registered MCP Servers')}</h4>

        {/* 全サーバーテストボタン */}
        {mcpServers.length > 0 && (
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault()
              e.stopPropagation()
              testAllConnections()
            }}
            disabled={testingAll || testingConnection !== null}
            className="text-xs px-2 py-1 bg-accent-tint hover:bg-accent-tint-strong text-accent rounded-control border border-accent flex items-center gap-1 disabled:opacity-50"
          >
            {testingAll ? (
              <div className="w-3 h-3 border-2 border-t-transparent border-accent rounded-full animate-spin mr-1"></div>
            ) : (
              <FiZap className="w-3 h-3 mr-1" />
            )}
            {testingAll ? t('Testing...') : t('Test All Servers')}
          </button>
        )}
      </div>

      {/* テスト結果の概要表示 */}
      <ConnectionTestResults
        connectionResults={connectionResults}
        mcpServers={mcpServers}
        onClearResults={clearConnectionResults}
      />

      {/* サーバーリスト */}
      <div
        className="border border-subtle rounded-control divide-y divide-subtle"
        onClick={preventModalClose}
      >
        {mcpServers.map((server) => (
          <ServerListItem
            key={server.name}
            server={server}
            onEdit={onEdit}
            onDelete={onDelete}
            testServerConnection={testServerConnection}
            testingConnection={testingConnection}
            connectionResult={connectionResults[server.name]}
          />
        ))}
      </div>
    </div>
  )
}

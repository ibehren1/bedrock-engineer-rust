import React from 'react'
import { useTranslation } from 'react-i18next'
import { McpServerConfig } from '@/types/agent-chat'
import { ConnectionTestResult } from './types/mcpServer.types'
import { FiEdit, FiTrash2, FiZap } from 'react-icons/fi'
import { preventModalClose } from './utils/eventUtils'

interface ServerListItemProps {
  server: McpServerConfig
  onEdit: (serverName: string) => void
  onDelete: (serverName: string) => void
  testServerConnection: (serverName: string) => Promise<void>
  testingConnection: string | null
  connectionResult?: ConnectionTestResult
}

/**
 * 個別のサーバー項目を表示するコンポーネント
 */
export const ServerListItem: React.FC<ServerListItemProps> = ({
  server,
  onEdit,
  onDelete,
  testServerConnection,
  testingConnection,
  connectionResult
}) => {
  const { t } = useTranslation()

  return (
    <div className="p-3 hover:bg-surface-2" onClick={preventModalClose}>
      {/* サーバー情報表示 */}
      <div className="flex justify-between items-start">
        <div>
          <h5 className="font-medium text-sm flex items-center text-ink">
            {server.name}
            {testingConnection === server.name && (
              <div className="ml-2 w-3 h-3 border-2 border-t-transparent border-accent rounded-full animate-spin"></div>
            )}
          </h5>
          <p className="text-xs text-ink-muted">{server.description}</p>
          <p className="text-xs font-mono text-ink-muted mt-1">
            <code className="text-ink-muted">
              {server.connectionType === 'url' ? (
                server.url
              ) : (
                <>
                  {server.command} {server.args?.join(' ') || ''}
                </>
              )}
            </code>
          </p>

          {/* ヘッダー情報の表示（URL形式で、headersがある場合のみ） */}
          {server.connectionType === 'url' &&
            server.headers &&
            Object.keys(server.headers).length > 0 && (
              <div className="mt-1 text-xs">
                <p className="text-ink-muted">{t('Headers')}:</p>
                <div className="pl-2 mt-1 border-l-2 border-subtle">
                  {Object.entries(server.headers).map(([key, value]) => (
                    <div key={key} className="font-mono">
                      <span className="text-accent">{key}</span>:{' '}
                      <span className="text-ink-muted">
                        {key.toLowerCase() === 'authorization'
                          ? `${value.substring(0, 10)}...`
                          : `${value}`}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

          {/* 接続テスト結果表示 */}
          {connectionResult && (
            <div
              className={`mt-2 p-2 rounded-control text-xs ${
                connectionResult.success
                  ? 'bg-success-soft border border-success'
                  : 'bg-danger-soft border border-danger'
              }`}
            >
              <div
                className={`font-medium mb-1 flex items-center ${
                  connectionResult.success ? 'text-success' : 'text-danger'
                }`}
              >
                <span
                  className={`inline-block w-2 h-2 mr-1 rounded-full ${
                    connectionResult.success ? 'bg-success-soft' : 'bg-danger-soft'
                  }`}
                ></span>
                {connectionResult.success ? t('Connection Successful') : t('Connection Failed')}
                <span className="ml-2 font-normal text-ink-muted">
                  {new Date(connectionResult.testedAt).toLocaleTimeString()}
                </span>
              </div>

              {connectionResult.success ? (
                // 成功時の詳細表示
                <div>
                  <div className="text-success">
                    {connectionResult.details?.toolCount || 0} {t('tools available')}
                  </div>
                  {connectionResult.details?.startupTime !== undefined && (
                    <div className="text-ink-muted mt-1">
                      {t('Startup time')}: {connectionResult.details?.startupTime}ms
                    </div>
                  )}
                </div>
              ) : (
                // 失敗時の詳細表示
                <div>
                  <div className="text-danger">{connectionResult.details?.error}</div>
                  {connectionResult.details?.errorDetails && (
                    <div className="mt-1 text-ink p-1 bg-raised rounded-control">
                      <strong>{t('Solution')}:</strong> {connectionResult.details?.errorDetails}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        {/* アクションボタン */}
        <div className="flex space-x-2">
          {/* テストボタン */}
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault()
              e.stopPropagation()
              testServerConnection(server.name)
            }}
            disabled={testingConnection !== null}
            className="p-1 text-ink-muted hover:text-accent disabled:opacity-50"
            title={t('Test Connection')}
          >
            <FiZap size={18} />
          </button>

          {/* 編集ボタン */}
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault()
              e.stopPropagation()
              onEdit(server.name)
            }}
            className="p-1 text-ink-muted hover:text-accent"
            title={t('Edit Server')}
          >
            <FiEdit size={18} />
          </button>

          {/* 削除ボタン */}
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault()
              e.stopPropagation()
              onDelete(server.name)
            }}
            className="p-1 text-ink-muted hover:text-danger-strong"
            title={t('Delete Server')}
          >
            <FiTrash2 size={18} />
          </button>
        </div>
      </div>
    </div>
  )
}

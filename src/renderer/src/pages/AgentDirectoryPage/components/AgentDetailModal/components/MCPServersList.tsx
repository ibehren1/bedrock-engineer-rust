import React from 'react'
import { useTranslation } from 'react-i18next'
import { McpServerConfig } from '@/types/agent-chat'

interface MCPServersListProps {
  servers: McpServerConfig[]
}

export const MCPServersList: React.FC<MCPServersListProps> = ({ servers }) => {
  const { t } = useTranslation()

  if (!servers || servers.length === 0) {
    return null
  }

  return (
    <div>
      <h3 className="text-heading font-medium mb-2 text-ink">{t('mcpServersLabel')}</h3>
      <div className="space-y-3">
        {servers.map((server, index) => (
          <div key={index} className="p-2.5 bg-surface-2 rounded-container border border-subtle">
            <h4 className="font-medium text-sm text-ink mb-2">{server.name}</h4>
            {server.description && (
              <p className="text-sm text-ink-muted mb-2">{server.description}</p>
            )}
            <div className="text-xs bg-raised p-2 rounded-control font-mono text-ink">
              {server.connectionType === 'url' || server.url ? (
                <>
                  <div>
                    <span className="text-accent">URL:</span> {server.url}
                  </div>
                  {server.headers && Object.keys(server.headers).length > 0 && (
                    <div>
                      <span className="text-accent">Headers:</span>{' '}
                      {Object.keys(server.headers).join(', ')}
                    </div>
                  )}
                </>
              ) : (
                <>
                  <div>
                    <span className="text-accent">Command:</span> {server.command}
                  </div>
                  {server.args && server.args.length > 0 && (
                    <div>
                      <span className="text-accent">Args:</span> {server.args.join(' ')}
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

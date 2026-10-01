import React from 'react'
import { useTranslation } from 'react-i18next'
import { FiPlus, FiTrash2, FiMessageSquare } from 'react-icons/fi'

interface SessionListProps {
  sessions: string[]
  selectedSessionId: string | null
  onSessionSelect: (sessionId: string) => void
  onCreateSession: () => void
  onDeleteSession: (sessionId: string) => void
}

export const SessionList: React.FC<SessionListProps> = ({
  sessions,
  selectedSessionId,
  onSessionSelect,
  onCreateSession,
  onDeleteSession
}) => {
  const { t } = useTranslation()
  const formatSessionId = (sessionId: string) => {
    // タイムスタンプを含む場合は日時を表示
    if (sessionId.includes('-') && sessionId.match(/\d{13}$/)) {
      const timestamp = sessionId.split('-').pop()
      if (timestamp) {
        const date = new Date(parseInt(timestamp))
        return date.toLocaleString('ja-JP', {
          month: 'short',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit'
        })
      }
    }
    return sessionId.length > 20 ? sessionId.substring(0, 20) + '...' : sessionId
  }

  return (
    <div className="w-80 bg-surface border-r border-subtle flex flex-col">
      {/* ヘッダー */}
      <div className="p-2.5 border-b border-subtle">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-heading font-semibold text-ink">
            {t('backgroundAgent.sessions.title')}
          </h2>
          <span className="text-sm text-ink-muted">{sessions.length}</span>
        </div>

        {/* 新規セッション作成ボタン */}
        <button
          onClick={onCreateSession}
          className="w-full flex items-center justify-center gap-2 px-2.5 py-1 bg-accent text-accent-fg rounded-control hover:bg-accent-strong transition-colors"
        >
          <FiPlus className="w-4 h-4" />
          {t('backgroundAgent.sessions.newSession')}
        </button>
      </div>

      {/* セッション一覧 */}
      <div className="flex-1 overflow-y-auto">
        {sessions.length === 0 ? (
          <div className="p-2.5 text-center">
            <FiMessageSquare className="w-12 h-12 text-ink-faint mx-auto mb-3" />
            <p className="text-ink-muted text-sm">
              {t('backgroundAgent.sessions.noSessions')}
              <br />
              {t('backgroundAgent.sessions.createNewSession')}
            </p>
          </div>
        ) : (
          <div className="p-2 space-y-1">
            {sessions.map((sessionId) => (
              <div
                key={sessionId}
                className={`group relative flex items-center p-3 rounded-container cursor-pointer transition-colors ${
                  selectedSessionId === sessionId
                    ? 'bg-accent-tint border border-accent'
                    : 'hover:bg-surface-2'
                }`}
                onClick={() => onSessionSelect(sessionId)}
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <FiMessageSquare className="w-4 h-4 text-ink-faint flex-shrink-0" />
                    <span className="text-sm font-medium text-ink truncate">
                      {formatSessionId(sessionId)}
                    </span>
                  </div>
                  <p className="text-xs text-ink-muted mt-1 truncate">{sessionId}</p>
                </div>

                {/* 削除ボタン */}
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    if (window.confirm(t('backgroundAgent.sessions.confirmDelete'))) {
                      onDeleteSession(sessionId)
                    }
                  }}
                  className="opacity-0 group-hover:opacity-100 transition-opacity p-1 text-ink-faint hover:text-danger-strong"
                  title={t('backgroundAgent.sessions.deleteSession')}
                >
                  <FiTrash2 className="w-4 h-4" />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

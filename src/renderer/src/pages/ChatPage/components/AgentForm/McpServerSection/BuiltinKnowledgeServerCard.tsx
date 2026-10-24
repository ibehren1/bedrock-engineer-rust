import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FiDatabase, FiZap } from 'react-icons/fi'
import type { LocalKnowledgeBaseRef } from '@/types/agent-chat'
import {
  KNOWLEDGE_BASE_TOOL_DEFINITIONS,
  localKnowledgeBaseServer
} from '@/common/agents/knowledgeTools'

type TestResult = { success: boolean; message: string; details?: { toolNames?: string[] } }

/**
 * The built-in `local-knowledgebase` server, shown read-only above the agent's own MCP servers
 * while the agent has local knowledgebases (it is attached automatically, never stored).
 */
export const BuiltinKnowledgeServerCard: React.FC<{
  agentId: string
  knowledgeBases: LocalKnowledgeBaseRef[]
}> = ({ agentId, knowledgeBases }) => {
  const { t } = useTranslation()
  const [testing, setTesting] = useState(false)
  const [result, setResult] = useState<TestResult | null>(null)
  const server = localKnowledgeBaseServer(agentId)

  const test = async () => {
    setTesting(true)
    try {
      setResult(await window.api.mcp.testConnection(server))
    } catch (e) {
      setResult({ success: false, message: e instanceof Error ? e.message : String(e) })
    } finally {
      setTesting(false)
    }
  }

  return (
    <div className="border border-subtle rounded-control p-3 bg-surface-2 space-y-2">
      <div className="flex justify-between items-start gap-2">
        <div>
          <h5 className="font-medium text-sm flex items-center gap-1.5 text-ink">
            <FiDatabase className="w-4 h-4" />
            {server.name}
            <span className="text-[10px] uppercase tracking-wide px-1.5 py-0.5 rounded-control border border-accent text-accent">
              {t('knowledgeBase.builtinServer.badge')}
            </span>
          </h5>
          <p className="text-xs text-ink-muted mt-0.5">
            {t('knowledgeBase.builtinServer.description', {
              names: knowledgeBases.map((k) => k.name).join(', ')
            })}
          </p>
        </div>
        <button
          type="button"
          onClick={test}
          disabled={testing}
          className="text-xs px-2 py-1 bg-accent-tint hover:bg-accent-tint-strong text-accent rounded-control border border-accent flex items-center gap-1 disabled:opacity-50 whitespace-nowrap"
        >
          {testing ? (
            <div className="w-3 h-3 border-2 border-t-transparent border-accent rounded-full animate-spin" />
          ) : (
            <FiZap className="w-3 h-3" />
          )}
          {testing ? t('Testing...') : t('knowledgeBase.builtinServer.test')}
        </button>
      </div>
      <ul className="space-y-1">
        {KNOWLEDGE_BASE_TOOL_DEFINITIONS.map((tool) => (
          <li key={tool.name} className="text-xs">
            <code className="text-accent">{tool.name}</code>
            <span className="text-ink-muted"> — {tool.description}</span>
          </li>
        ))}
      </ul>
      {result && (
        <p className={`text-xs ${result.success ? 'text-success' : 'text-danger'}`}>
          {result.success
            ? t('knowledgeBase.builtinServer.connected', {
                count: result.details?.toolNames?.length ?? 0
              })
            : result.message}
        </p>
      )}
    </div>
  )
}

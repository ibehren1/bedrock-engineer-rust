import React from 'react'

interface ExecuteCommandResponse {
  success: boolean
  name: string
  message: string
  stdout?: string
  stderr?: string
  exitCode?: number
  processInfo?: {
    pid: number
    command: string
    detached: boolean
  }
}

export const ExecuteCommandResult: React.FC<{ response: ExecuteCommandResponse }> = ({
  response
}) => {
  const removeAnsiCodes = (text: string) => {
    // ANSI エスケープシーケンスを削除
    return text.replace(/\u001b\[\??\d*[a-zA-Z]/g, '')
  }

  return (
    <div className="bg-surface-2 text-ink p-2.5 rounded-container overflow-x-auto shadow-raised border border-subtle">
      {/* Command Info */}
      {response.processInfo?.command && (
        <div className="mb-2">
          <span className="text-success">$ </span>
          <span className="font-mono">{response.processInfo.command}</span>
        </div>
      )}

      {/* Process ID */}
      {response.processInfo?.pid && (
        <div className="text-sm text-ink-faint mb-2">
          Process ID: {response.processInfo.pid}
          {response.processInfo.detached && ' (detached)'}
        </div>
      )}

      {/* Output */}
      {(response.stdout || response.stderr) && (
        <div className="font-mono whitespace-pre-wrap">
          {response.stdout && <div className="text-ink">{removeAnsiCodes(response.stdout)}</div>}
          {response.stderr && response.stderr !== '\u001b[?1034h' && (
            <div className="text-danger">{removeAnsiCodes(response.stderr)}</div>
          )}
        </div>
      )}

      {/* Exit Code */}
      {response.exitCode !== undefined && (
        <div className={`text-sm mt-2 ${response.exitCode === 0 ? 'text-success' : 'text-danger'}`}>
          Exit Code: {response.exitCode}
        </div>
      )}
    </div>
  )
}

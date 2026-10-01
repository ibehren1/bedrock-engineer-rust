import React, { useRef } from 'react'
import Editor, { Monaco, OnMount } from '@monaco-editor/react'
import '@renderer/lib/monaco/setup'
import { useTheme } from '@renderer/hooks/useTheme'

interface MonacoJSONEditorProps {
  value: string
  onChange: (value: string) => void
  height?: string
  onValidate?: (isValid: boolean, errors?: any[]) => void
  className?: string
  readOnly?: boolean
}

export const MonacoJSONEditor: React.FC<MonacoJSONEditorProps> = ({
  value,
  onChange,
  height = '300px',
  onValidate,
  className = '',
  readOnly = false
}) => {
  const { isDarkMode } = useTheme()
  const editorRef = useRef<any>(null)

  // エディタの設定
  const handleEditorDidMount: OnMount = (editor, monaco) => {
    editorRef.current = editor

    // 初期のJSONを検証
    if (value && onValidate) {
      validateJSON(value)
    }

    // カスタムJSONスキーマの適用（可能であれば）
    setupJSONValidation(monaco)
  }

  // JSONの検証
  const validateJSON = (code: string | undefined) => {
    if (!code || !onValidate) return

    try {
      JSON.parse(code)
      onValidate(true)
    } catch (error) {
      onValidate(false, [error])
    }
  }

  // コードが変更されたときの処理
  const handleEditorChange = (value: string | undefined) => {
    if (value !== undefined) {
      onChange(value)
      validateJSON(value)
    }
  }

  // JSONスキーマの設定（MCPサーバー用）
  const setupJSONValidation = (monaco: Monaco) => {
    // モナコにJSONスキーマを登録
    // monaco-editor >= 0.55 exports the JSON defaults as `monaco.json`; older builds had them
    // under `monaco.languages.json`.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const json = (monaco as any).json ?? (monaco.languages as any).json
    json?.jsonDefaults.setDiagnosticsOptions({
      validate: true,
      schemas: [
        {
          uri: 'http://myserver/mcp-server-schema.json', // この URI は単なる識別子
          fileMatch: ['*'], // すべてのファイルに適用
          schema: {
            type: 'object',
            properties: {
              mcpServers: {
                type: 'object',
                additionalProperties: {
                  type: 'object',
                  required: ['command', 'args'],
                  properties: {
                    command: { type: 'string' },
                    args: {
                      type: 'array',
                      items: { type: 'string' }
                    },
                    env: {
                      type: 'object',
                      additionalProperties: { type: 'string' }
                    }
                  }
                }
              }
            }
          }
        }
      ]
    })
  }

  // フォーマットボタンのクリックハンドラ
  const formatJSON = () => {
    if (!editorRef.current) return

    try {
      const unformatted = editorRef.current.getValue()
      const parsed = JSON.parse(unformatted)
      const formatted = JSON.stringify(parsed, null, 2)
      editorRef.current.setValue(formatted)
    } catch (error) {
      // エラーの場合は何もしない（バリデーションが処理する）
    }
  }

  return (
    <div className={`relative ${className}`}>
      <Editor
        height={height}
        language="json"
        value={value}
        theme={isDarkMode ? 'vs-dark' : 'light'}
        options={{
          minimap: { enabled: false },
          lineNumbers: 'on',
          scrollBeyondLastLine: false,
          automaticLayout: true,
          tabSize: 2,
          folding: true,
          fontSize: 13,
          wordWrap: 'on',
          readOnly: readOnly
        }}
        onMount={handleEditorDidMount}
        onChange={handleEditorChange}
      />
      {!readOnly && (
        <button
          type="button"
          onClick={formatJSON}
          className="absolute top-2 right-2 z-10 text-xs bg-raised
          hover:bg-sunken text-ink py-1 px-2 rounded-control"
          title={'Format JSON'}
        >
          Format
        </button>
      )}
    </div>
  )
}

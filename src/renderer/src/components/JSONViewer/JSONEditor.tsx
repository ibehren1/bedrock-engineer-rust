import React, { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'

interface JSONEditorProps {
  value: any
  onChange: (value: any) => void
  height?: string
  error?: string
  defaultValue?: any
}

const JSONEditor: React.FC<JSONEditorProps> = ({
  value,
  onChange,
  height = '200px',
  error,
  defaultValue = {}
}) => {
  const { t } = useTranslation()
  const [jsonText, setJsonText] = useState('')
  const [localError, setLocalError] = useState('')

  // valueが変更されたときにテキスト表示を更新
  useEffect(() => {
    try {
      const formatted = JSON.stringify(value || defaultValue, null, 2)
      setJsonText(formatted)
      setLocalError('')
    } catch (e) {
      setLocalError('Invalid JSON structure')
    }
  }, [value, defaultValue])

  // テキストが変更されたときの処理
  const handleTextChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value
    setJsonText(text)

    try {
      const parsedValue = JSON.parse(text)
      onChange(parsedValue)
      setLocalError('')
    } catch (e) {
      setLocalError('Invalid JSON syntax')
      // エラーがあってもテキスト自体の更新は許可
    }
  }

  return (
    <div>
      <textarea
        value={jsonText}
        onChange={handleTextChange}
        className={`w-full p-2 text-sm font-mono border rounded-control resize-y ${
          localError || error ? 'border-danger' : 'bg-surface border-subtle'
        } text-ink`}
        style={{ height }}
        placeholder={t('Enter JSON schema here')}
      />
      {(localError || error) && <p className="text-xs text-danger mt-1">{localError || error}</p>}
    </div>
  )
}

export default JSONEditor

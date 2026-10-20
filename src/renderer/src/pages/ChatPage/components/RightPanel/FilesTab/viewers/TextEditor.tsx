import React, { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { detectPlatform } from '@renderer/lib/nativeMenus'

type TextEditorProps = {
  value: string
  onChange: (value: string) => void
  onSave: () => void
  onCancel: () => void
  dirty: boolean
  saving: boolean
}

/** Plain monospace editor for text files; Cmd/Ctrl+S saves. */
export const TextEditor: React.FC<TextEditorProps> = ({
  value,
  onChange,
  onSave,
  onCancel,
  dirty,
  saving
}) => {
  const { t } = useTranslation()
  const ref = useRef<HTMLTextAreaElement>(null)
  const mac = detectPlatform(navigator.userAgent) === 'mac'

  useEffect(() => {
    ref.current?.focus()
  }, [])

  return (
    <div className="h-full flex flex-col gap-2 p-2">
      <textarea
        ref={ref}
        value={value}
        spellCheck={false}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          const mod = mac ? e.metaKey : e.ctrlKey
          if (mod && !e.altKey && e.key.toLowerCase() === 's') {
            e.preventDefault()
            if (dirty && !saving) onSave()
          } else if (e.key === 'Tab' && !e.shiftKey && !mod) {
            // Insert a tab instead of leaving the editor.
            e.preventDefault()
            const el = e.currentTarget
            const { selectionStart, selectionEnd } = el
            const next = `${value.slice(0, selectionStart)}\t${value.slice(selectionEnd)}`
            onChange(next)
            requestAnimationFrame(() => {
              el.selectionStart = el.selectionEnd = selectionStart + 1
            })
          }
        }}
        className="flex-1 min-h-0 w-full resize-none rounded-container border border-accent bg-surface-2 p-2.5 font-mono text-xs text-ink focus:outline-none focus:ring-1 focus:ring-accent"
      />
      <div className="flex items-center gap-2 shrink-0">
        <span className="text-micro text-ink-faint mr-auto">
          {t('filesPanel.saveShortcut', { key: mac ? '⌘' : 'Ctrl' })}
        </span>
        <button
          onClick={onCancel}
          className="px-2.5 py-1 text-xs rounded-control text-ink-muted hover:bg-raised hover:text-ink"
        >
          {t('filesPanel.cancel')}
        </button>
        <button
          onClick={onSave}
          disabled={!dirty || saving}
          className="px-2.5 py-1 text-xs rounded-control bg-accent text-accent-fg hover:bg-accent-strong disabled:opacity-50"
        >
          {t('filesPanel.save')}
        </button>
      </div>
    </div>
  )
}

export default TextEditor

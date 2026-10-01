import React, { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  FiFile,
  FiFileText,
  FiFolder,
  FiImage,
  FiPaperclip,
  FiPlus,
  FiTrash2
} from 'react-icons/fi'
import type { ChatAttachment } from '../../hooks/useChatAttachments'
import { folderName } from '../../lib/folderName'

type AttachmentsButtonProps = {
  files: ChatAttachment[]
  directory: string
  totalSize: number
  isBusy: boolean
  onAdd: () => void
  onRemove: (name: string) => void
  onOpenFolder: () => void
  onOpen: () => void
}

const formatSize = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

const kindIcon = (kind: ChatAttachment['kind']) => {
  if (kind === 'image') return <FiImage className="w-4 h-4 shrink-0" />
  if (kind === 'other') return <FiFile className="w-4 h-4 shrink-0" />
  return <FiFileText className="w-4 h-4 shrink-0" />
}

/**
 * Paperclip menu for the chat toolbar: the files attached to this chat, with per-file delete,
 * a native picker and a way into the folder itself.
 *
 * Opens upward because the toolbar sits at the bottom of the window — same approach as
 * SandboxButton. Always rendered so files can be attached before the first message.
 */
export const AttachmentsButton: React.FC<AttachmentsButtonProps> = ({
  files,
  directory,
  totalSize,
  isBusy,
  onAdd,
  onRemove,
  onOpenFolder,
  onOpen
}) => {
  const { t } = useTranslation()
  const [isOpen, setIsOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const toggle = () => {
    // Pick up anything added or removed outside the app before showing the list.
    if (!isOpen) onOpen()
    setIsOpen((open) => !open)
  }

  const act = (action: () => void) => {
    setIsOpen(false)
    action()
  }

  return (
    <div className="relative" ref={containerRef}>
      <button
        onClick={toggle}
        disabled={isBusy}
        title={t('attachments.menu.title')}
        aria-label={t('attachments.menu.title')}
        className={`relative p-2 rounded-full hover:bg-surface transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed ${
          files.length > 0 ? 'text-accent' : 'text-ink-faint'
        }`}
      >
        <FiPaperclip className={isBusy ? 'animate-pulse' : ''} />
        {files.length > 0 && (
          <span className="absolute -top-0.5 -right-0.5 min-w-4 h-4 px-1 rounded-full bg-accent text-accent-fg text-[10px] leading-4 text-center">
            {files.length}
          </span>
        )}
      </button>

      {isOpen && (
        <div className="absolute right-0 z-20 w-80 bottom-full mb-1 bg-surface rounded-container shadow-lg border border-subtle py-1">
          <div className="px-2.5 py-1 border-b border-subtle">
            <span className="text-sm font-medium text-ink">{t('attachments.menu.title')}</span>

            {directory && (
              <p className="mt-1 text-xs font-mono text-ink-muted truncate" title={directory}>
                {folderName(directory)}
              </p>
            )}

            <p className="mt-1 text-xs text-ink-muted">
              {files.length > 0
                ? t('attachments.menu.summary', {
                    count: files.length,
                    size: formatSize(totalSize)
                  })
                : t('attachments.menu.empty')}
            </p>
          </div>

          {files.length > 0 && (
            <div className="max-h-64 overflow-y-auto border-b border-subtle">
              {files.map((file) => (
                <div
                  key={file.name}
                  className="group flex items-center gap-2 px-2.5 py-1 text-sm text-ink hover:bg-raised"
                >
                  {kindIcon(file.kind)}
                  <span className="flex-1 truncate" title={file.path}>
                    {file.name}
                  </span>
                  <span className="text-xs text-ink-muted">{formatSize(file.size)}</span>
                  <button
                    onClick={() => onRemove(file.name)}
                    title={t('attachments.menu.remove', { name: file.name })}
                    aria-label={t('attachments.menu.remove', { name: file.name })}
                    className="text-danger opacity-0 group-hover:opacity-100 transition-opacity"
                  >
                    <FiTrash2 className="w-4 h-4" />
                  </button>
                </div>
              ))}
            </div>
          )}

          <button
            onClick={() => act(onAdd)}
            className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
          >
            <FiPlus className="w-4 h-4" />
            {t('attachments.menu.addFiles')}
          </button>

          <button
            onClick={() => act(onOpenFolder)}
            className="w-full text-left px-2.5 py-1 text-sm text-ink hover:bg-raised flex items-center gap-2"
          >
            <FiFolder className="w-4 h-4" />
            {t('attachments.menu.openFolder')}
          </button>
        </div>
      )}
    </div>
  )
}

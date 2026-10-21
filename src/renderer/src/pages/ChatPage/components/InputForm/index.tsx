import React, { useState } from 'react'
import { AttachedImage, TextArea } from './TextArea'
import { ToolSettings } from './ToolSettings'
import { AttachmentsButton } from './AttachmentsButton'
import { DirectorySelector } from './DirectorySelector'
import { SandboxButton } from './SandboxButton'
import { MemoryToggle, type MemoryToggleProps } from './MemoryToggle'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'
import type { ChatAttachment } from '../../hooks/useChatAttachments'
import { SendMsgKey } from '@/types/agent-chat'
import { FiFolder, FiStopCircle } from 'react-icons/fi'
import {
  TbMarkdown,
  TbMessagePlus,
  TbFileTypeDocx,
  TbFileTypePdf,
  TbTerminal2
} from 'react-icons/tb'
import { useTranslation } from 'react-i18next'

type InputFormProps = {
  userInput: string
  loading: boolean
  projectPath?: string
  sendMsgKey?: SendMsgKey
  onSubmit: (input: string, attachedImages: AttachedImage[]) => void
  onChange: (input: string) => void
  onOpenToolSettings: () => void
  onSelectDirectory: () => void
  onOpenIgnoreModal: () => void
  onClearChat: () => void
  onExportChat?: () => void // Markdown エクスポートのハンドラ
  isExporting?: boolean
  onExportWord?: () => void // Word (.docx) エクスポートのハンドラ
  isExportingWord?: boolean
  onExportPdf?: () => void // PDF エクスポートのハンドラ
  isExportingPdf?: boolean
  onStopGeneration?: () => void // 停止ボタンのハンドラ
  hasMessages: boolean
  onHeightChange?: (height: number) => void // Text area height change handler
  sandbox?: {
    status: ChatSandboxStatus
    isBusy: boolean
    onStop: () => void
    onStart: () => void
    onRemove: (deleteData: boolean) => void
    onOpenFolder: () => void
    onOpenPanel: () => void
  }
  /** The Files button (switches to the Files tab), shown when a project folder is set. */
  filesPanel?: {
    isOpen: boolean
    onToggle: () => void
  }
  /** The Terminal button: opens a shell on this computer in the Terminal tab. */
  terminal?: {
    /** The Terminal tab is showing, so a click opens another shell. */
    isActive: boolean
    /** No chat session yet, so the shell has nowhere to belong. */
    disabled: boolean
    onOpen: () => void
  }
  attachments?: {
    files: ChatAttachment[]
    directory: string
    totalSize: number
    isBusy: boolean
    onAdd: () => void
    onRemove: (name: string) => void
    onOpenFolder: () => void
    onRefresh: () => void
    onAddFiles: (files: File[]) => Promise<void>
  }
  /** The per-chat "Use Memory Items" toggle; hidden when not given. */
  memoryToggle?: MemoryToggleProps
}

export const InputForm: React.FC<InputFormProps> = ({
  userInput,
  loading,
  projectPath = '',
  sendMsgKey = 'Enter',
  onSubmit,
  onChange,
  onOpenToolSettings,
  onSelectDirectory,
  onOpenIgnoreModal,
  onClearChat,
  onExportChat,
  isExporting,
  onExportWord,
  isExportingWord,
  onExportPdf,
  isExportingPdf,
  onStopGeneration,
  hasMessages,
  onHeightChange,
  sandbox,
  filesPanel,
  terminal,
  attachments,
  memoryToggle
}) => {
  const [isComposing, setIsComposing] = useState(false)
  const { t } = useTranslation()

  // Sits at the bottom of the chat column, under whichever tab is showing.
  return (
    <div className="flex gap-2 pt-3">
      <div className="relative w-full">
        <div className="flex justify-between mb-2">
          {/* left */}
          <div className="flex flex-col justify-end gap-2 mb-1">
            <ToolSettings onOpenToolSettings={onOpenToolSettings} />
            <DirectorySelector
              projectPath={projectPath}
              onSelectDirectory={onSelectDirectory}
              onOpenIgnoreModal={onOpenIgnoreModal}
            />
          </div>

          {/* right */}
          <div className="flex items-end mb-1 gap-2">
            {/* Before the first message too: the setting is per chat and starts on. */}
            {memoryToggle && <MemoryToggle {...memoryToggle} />}
            {terminal && (
              <button
                onClick={terminal.onOpen}
                disabled={terminal.disabled}
                title={
                  terminal.disabled
                    ? t('filesPanel.terminalNoSession')
                    : terminal.isActive
                      ? t('filesPanel.terminalButtonNew')
                      : t('filesPanel.terminalButton')
                }
                aria-label={t('filesPanel.terminalButton')}
                className={`p-2 rounded-full hover:bg-surface transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed ${
                  terminal.isActive ? 'text-accent' : 'text-ink-muted hover:text-ink'
                }`}
              >
                <TbTerminal2 />
              </button>
            )}
            {/* Outside the hasMessages gate below: files have to be attachable before the
                first message is sent. */}
            {attachments && (
              <AttachmentsButton
                files={attachments.files}
                directory={attachments.directory}
                totalSize={attachments.totalSize}
                isBusy={attachments.isBusy}
                onAdd={attachments.onAdd}
                onRemove={attachments.onRemove}
                onOpenFolder={attachments.onOpenFolder}
                onOpen={attachments.onRefresh}
              />
            )}
            {hasMessages && (
              <>
                {loading && onStopGeneration && (
                  <button
                    title={t('Stop generation')}
                    aria-label={t('Stop generation')}
                    onClick={onStopGeneration}
                    className="p-2 text-danger hover:text-danger-strong rounded-full hover:bg-danger-soft transition-colors duration-200"
                  >
                    <FiStopCircle />
                  </button>
                )}
                {/* Only present once this chat actually has a sandbox to control */}
                {sandbox?.status.exists && (
                  <SandboxButton
                    status={sandbox.status}
                    isBusy={sandbox.isBusy}
                    onStop={sandbox.onStop}
                    onStart={sandbox.onStart}
                    onRemove={sandbox.onRemove}
                    onOpenFolder={sandbox.onOpenFolder}
                    onOpenPanel={sandbox.onOpenPanel}
                  />
                )}
              </>
            )}
            {/* Outside the hasMessages gate: the project's files can be browsed before the
                first message. */}
            {filesPanel && (
              <button
                title={t('filesPanel.toolbarButton')}
                onClick={filesPanel.onToggle}
                aria-label={t('filesPanel.toolbarButton')}
                aria-pressed={filesPanel.isOpen}
                className={`p-2 rounded-full hover:bg-surface transition-colors duration-200 ${
                  filesPanel.isOpen ? 'text-accent' : 'text-ink-muted hover:text-ink'
                }`}
              >
                <FiFolder />
              </button>
            )}
            {hasMessages && (
              <>
                {onExportChat && (
                  <button
                    title={t('Export chat to Markdown')}
                    aria-label={t('Export chat to Markdown')}
                    onClick={onExportChat}
                    disabled={isExporting}
                    className="p-2 text-ink-muted hover:text-ink rounded-full hover:bg-surface transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <TbMarkdown />
                  </button>
                )}
                {onExportWord && (
                  <button
                    title={t('Export chat to Word')}
                    aria-label={t('Export chat to Word')}
                    onClick={onExportWord}
                    disabled={isExportingWord}
                    className="p-2 text-ink-muted hover:text-ink rounded-full hover:bg-surface transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <TbFileTypeDocx />
                  </button>
                )}
                {onExportPdf && (
                  <button
                    title={t('Export chat to PDF')}
                    aria-label={t('Export chat to PDF')}
                    onClick={onExportPdf}
                    disabled={isExportingPdf}
                    className="p-2 text-ink-muted hover:text-ink rounded-full hover:bg-surface transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <TbFileTypePdf />
                  </button>
                )}
                <button
                  title={t('New chat')}
                  aria-label={t('New chat')}
                  onClick={onClearChat}
                  className="p-2 text-success hover:text-success-strong rounded-full hover:bg-success-soft transition-colors duration-200"
                >
                  <TbMessagePlus />
                </button>
              </>
            )}
          </div>
        </div>

        <TextArea
          value={userInput}
          onChange={onChange}
          disabled={loading}
          onSubmit={(userInput, attachedImages) => onSubmit(userInput, attachedImages)}
          isComposing={isComposing}
          setIsComposing={setIsComposing}
          sendMsgKey={sendMsgKey}
          onHeightChange={onHeightChange}
          onAddFiles={attachments?.onAddFiles}
        />
      </div>
    </div>
  )
}

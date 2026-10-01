import React, { useState } from 'react'
import { AttachedImage, TextArea } from './TextArea'
import { ToolSettings } from './ToolSettings'
import { AttachmentsButton } from './AttachmentsButton'
import { DirectorySelector } from './DirectorySelector'
import { SandboxButton } from './SandboxButton'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'
import type { ChatAttachment } from '../../hooks/useChatAttachments'
import { SendMsgKey } from '@/types/agent-chat'
import { FiStopCircle } from 'react-icons/fi'
import { TbMarkdown, TbMessagePlus, TbFileTypeDocx, TbFileTypePdf } from 'react-icons/tb'
import { useTranslation } from 'react-i18next'
import { Tooltip } from 'flowbite-react'

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
  isHistoryOpen?: boolean // Whether the chat history panel is expanded
  sandbox?: {
    status: ChatSandboxStatus
    isBusy: boolean
    onStop: () => void
    onStart: () => void
    onRemove: (deleteData: boolean) => void
    onOpenFolder: () => void
    onOpenPanel: () => void
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
  isHistoryOpen = false,
  sandbox,
  attachments
}) => {
  const [isComposing, setIsComposing] = useState(false)
  const { t } = useTranslation()

  // The form is fixed-positioned (offset past the app nav sidebar at 5rem).
  // When the history panel (w-96 = 24rem) opens, shift its left edge to match
  // so the input slides right in step with the message area above it.
  //
  // It keeps the full width when the sandbox panel is open: the panel stops above this
  // area instead, the same way the message list reserves room for it.
  return (
    <div
      className={`flex gap-2 fixed bottom-3 right-5 pt-3 transition-all duration-300 ease-in-out ${
        isHistoryOpen ? 'left-[29rem]' : 'left-[5rem]'
      }`}
    >
      <div className="relative w-full">
        <div className="flex justify-between mb-2">
          {/* left */}
          <div className="flex flex-col justify-end gap-2 mb-1">
            <div className="flex gap-4 items-center">
              <ToolSettings onOpenToolSettings={onOpenToolSettings} />
            </div>
            <DirectorySelector
              projectPath={projectPath}
              onSelectDirectory={onSelectDirectory}
              onOpenIgnoreModal={onOpenIgnoreModal}
            />
          </div>

          {/* right */}
          <div className="flex items-end mb-1 gap-2">
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
                  <Tooltip content={t('Stop generation')} placement="top" animation="duration-500">
                    <button
                      onClick={onStopGeneration}
                      className="p-2 text-danger hover:text-danger-strong rounded-full hover:bg-danger-soft transition-colors duration-200"
                    >
                      <FiStopCircle />
                    </button>
                  </Tooltip>
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
                {onExportChat && (
                  <Tooltip
                    content={t('Export chat to Markdown')}
                    placement="top"
                    animation="duration-500"
                  >
                    <button
                      onClick={onExportChat}
                      disabled={isExporting}
                      className="p-2 text-ink-muted hover:text-ink rounded-full hover:bg-surface transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <TbMarkdown />
                    </button>
                  </Tooltip>
                )}
                {onExportWord && (
                  <Tooltip
                    content={t('Export chat to Word')}
                    placement="top"
                    animation="duration-500"
                  >
                    <button
                      onClick={onExportWord}
                      disabled={isExportingWord}
                      className="p-2 text-ink-muted hover:text-ink rounded-full hover:bg-surface transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <TbFileTypeDocx />
                    </button>
                  </Tooltip>
                )}
                {onExportPdf && (
                  <Tooltip
                    content={t('Export chat to PDF')}
                    placement="top"
                    animation="duration-500"
                  >
                    <button
                      onClick={onExportPdf}
                      disabled={isExportingPdf}
                      className="p-2 text-ink-muted hover:text-ink rounded-full hover:bg-surface transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <TbFileTypePdf />
                    </button>
                  </Tooltip>
                )}
                <Tooltip content={t('New chat')} placement="top" animation="duration-500">
                  <button
                    onClick={onClearChat}
                    className="p-2 text-success hover:text-success-strong rounded-full hover:bg-success-soft transition-colors duration-200"
                  >
                    <TbMessagePlus />
                  </button>
                </Tooltip>
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

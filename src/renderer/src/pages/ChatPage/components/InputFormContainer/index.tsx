import React, { useState, memo, useImperativeHandle } from 'react'
import { InputForm } from '../InputForm'
import { AttachedImage } from '../InputForm/TextArea'
import { SendMsgKey } from '@/types/agent-chat'
import type { ChatSandboxStatus } from '../../hooks/useChatSandbox'
import type { ChatAttachment } from '../../hooks/useChatAttachments'

// InputFormContainer の参照タイプを定義
export type InputFormContainerRef = {
  setInputText: (text: string) => void
}

type InputFormContainerProps = {
  loading: boolean
  projectPath?: string
  sendMsgKey?: SendMsgKey
  onSubmit: (input: string, attachedImages: AttachedImage[]) => void
  onOpenToolSettings: () => void
  onSelectDirectory: () => void
  onOpenIgnoreModal: () => void
  onClearChat: () => void
  onExportChat?: () => void
  isExporting?: boolean
  onExportWord?: () => void
  isExportingWord?: boolean
  onExportPdf?: () => void
  isExportingPdf?: boolean
  onStopGeneration?: () => void
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

/**
 * 入力フォームを管理する独立したコンテナコンポーネント
 * 内部で入力状態を管理し、親コンポーネントへの不要な再レンダリングを防止する
 */
const InputFormContainer = React.forwardRef<InputFormContainerRef, InputFormContainerProps>(
  (
    {
      loading,
      projectPath = '',
      sendMsgKey = 'Enter',
      onSubmit,
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
      isHistoryOpen,
      sandbox,
      attachments
    },
    ref
  ) => {
    // ローカルで入力状態を管理
    const [localUserInput, setLocalUserInput] = useState('')

    // 外部からテキスト入力を設定するためのメソッドを公開
    useImperativeHandle(ref, () => ({
      setInputText: (text: string) => {
        setLocalUserInput(text)
      }
    }))

    // 送信処理 - 親コンポーネントに値を渡す
    const handleSubmit = (input: string, attachedImages: AttachedImage[]) => {
      onSubmit(input, attachedImages)
      setLocalUserInput('') // 送信後に入力をクリア
    }

    // チャットクリア処理 - ローカル入力もクリア
    const handleClearChat = () => {
      onClearChat()
      setLocalUserInput('')
    }

    return (
      <InputForm
        userInput={localUserInput}
        loading={loading}
        projectPath={projectPath}
        sendMsgKey={sendMsgKey}
        onSubmit={handleSubmit}
        onChange={setLocalUserInput}
        onOpenToolSettings={onOpenToolSettings}
        onSelectDirectory={onSelectDirectory}
        onOpenIgnoreModal={onOpenIgnoreModal}
        onClearChat={handleClearChat}
        onExportChat={onExportChat}
        isExporting={isExporting}
        onExportWord={onExportWord}
        isExportingWord={isExportingWord}
        onExportPdf={onExportPdf}
        isExportingPdf={isExportingPdf}
        onStopGeneration={onStopGeneration}
        hasMessages={hasMessages}
        onHeightChange={onHeightChange}
        isHistoryOpen={isHistoryOpen}
        sandbox={sandbox}
        attachments={attachments}
      />
    )
  }
)

// 表示名を設定（開発者ツール用）
InputFormContainer.displayName = 'InputFormContainer'

// React.memo でコンポーネントをメモ化
export default memo(InputFormContainer)

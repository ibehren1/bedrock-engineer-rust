import { useEffect, useState, useCallback, useRef, useMemo } from 'react'
import AILogo from '../../assets/images/icons/bedrock-color.png'
import { MessageList } from './components/MessageList'
import InputFormContainer, { InputFormContainerRef } from './components/InputFormContainer'
import { ExampleScenarios } from './components/ExampleScenarios'
import { useAgentChat } from './hooks/useAgentChat'
import useSetting from '@renderer/hooks/useSetting'
import { IgnoreSettingsModal } from '@renderer/components/IgnoreSettingsModal'
import { useToolSettingModal } from './modals/useToolSettingModal'
import { FiChevronRight, FiBarChart2 } from 'react-icons/fi'
import { FaListCheck } from 'react-icons/fa6'
import { Tooltip } from 'flowbite-react'
import { useTranslation } from 'react-i18next'
import { AttachedImage } from './components/InputForm/TextArea'
import { ChatHistory } from './components/ChatHistory'
import { useSystemPromptModal } from './modals/useSystemPromptModal'
import { useTokenAnalyticsModal, calculateAnalytics } from './modals/useTokenAnalyticsModal'
import { useTodoModal } from './modals/useTodoModal'
import { HostCommandApprovalModal } from './modals/HostCommandApprovalModal'
import { useChatSandbox } from './hooks/useChatSandbox'
import { SandboxPanel } from './components/SandboxPanel'
import { useChatAttachments } from './hooks/useChatAttachments'
import { useChatHistory } from '@renderer/contexts/ChatHistoryContext'
import { useLocation, useNavigate } from 'react-router-dom'
import { useStreamingAutoScroll } from '@renderer/hooks/useStreamingAutoScroll'
import { useLightProcessingModel } from '@renderer/lib/modelSelection'
import { generateSessionTitle } from './utils/titleGenerator'
import { buildChatMarkdown } from './utils/chatExport'
import { buildChatHtml } from './utils/chatDocxExport'
import { buildChatPdfHtml } from './utils/chatPdfExport'
import { avatarDataUrl } from './utils/chatHtmlExport'
import { DrawioRasterizer, DrawioRasterizerRef } from './utils/DrawioRasterizer'
import { allModels } from '@common/models/models'
import { IdentifiableMessage } from '@/types/chat/message'
import toast from 'react-hot-toast'
import { HELP_AGENT_ID, HELP_AGENT_SYSTEM_PROMPT, HELP_CHAT_TITLE } from './constants/helpAgent'
import { HelpSessionProvider } from './lib/helpSession'

export default function ChatPage() {
  const { t } = useTranslation()
  const location = useLocation()
  const navigate = useNavigate()
  const {
    currentLLM: llm,
    projectPath,
    selectDirectory,
    sendMsgKey,
    selectedAgentId,
    setSelectedAgentId,
    agents,
    currentAgent,
    currentAgentSystemPrompt: systemPrompt,
    availableModels,
    userEmoji,
    userName
  } = useSetting()

  const inputFormRef = useRef<InputFormContainerRef>(null)

  const { deleteMessage, getSession, updateSessionTitle, createSession, sessions } =
    useChatHistory()
  const { getLightModelId } = useLightProcessingModel()

  // Help チャット。セッション自身の agentId から判定するので、履歴から開き直しても、
  // アプリを再起動しても Help のままになる。
  const [isHelpSession, setIsHelpSession] = useState(false)
  // 添付できなかった場合だけ使うユーザーガイド本文（システムプロンプトに載せる）。
  const [helpGuideText, setHelpGuideText] = useState<string>()

  const helpAgent = agents.find((agent) => agent.id === HELP_AGENT_ID)

  // Help チャットではエージェント・モデル・システムプロンプトを差し替える。
  // setSelectedAgentId は永続化されるため呼ばない（通常のチャットが Help に固定されてしまう）。
  const activeAgentId = isHelpSession ? HELP_AGENT_ID : selectedAgentId
  const activeModelId = isHelpSession ? getLightModelId() : llm?.modelId
  const activeSystemPrompt = isHelpSession
    ? helpGuideText
      ? `${HELP_AGENT_SYSTEM_PROMPT}\n<user_guide>\n${helpGuideText}\n</user_guide>\n`
      : HELP_AGENT_SYSTEM_PROMPT
    : systemPrompt

  const currentScenarios = (isHelpSession ? helpAgent?.scenarios : currentAgent?.scenarios) || []

  const {
    messages,
    loading,
    reasoning,
    waitingForResponse,
    timeoutCountdown,
    heartbeatCount,
    handleSubmit,
    currentSessionId,
    setCurrentSessionId,
    clearChat,
    setMessages,
    stopGeneration
  } = useAgentChat(activeModelId, activeSystemPrompt, activeAgentId)

  // 送信ハンドラをuseCallbackでメモ化
  const onSubmit = useCallback(
    (input: string, images: AttachedImage[]) => {
      handleSubmit(input, images)
    },
    [handleSubmit]
  )

  // 表示中のセッションが Help かどうかを追随させる。
  useEffect(() => {
    const agentId = currentSessionId ? getSession(currentSessionId)?.agentId : undefined
    setIsHelpSession(agentId === HELP_AGENT_ID)
  }, [currentSessionId, getSession])

  // DrawIO diagrams render in an iframe, so a live component is needed to rasterize them.
  const drawioRasterizerRef = useRef<DrawioRasterizerRef>(null)
  const [isExporting, setIsExporting] = useState(false)
  const [isExportingWord, setIsExportingWord] = useState(false)
  const [isExportingPdf, setIsExportingPdf] = useState(false)

  // モデルIDから表示情報を解決する（チャットのアバターと同じロジック）。
  const resolveModel = useCallback(
    (modelId?: string) =>
      modelId
        ? availableModels.find((m) => m.modelId === modelId) ??
          allModels.find((m) => m.modelId === modelId)
        : undefined,
    [availableModels]
  )

  // 見出しは「Assistant – <モデルID>」「User – <ユーザー名>」の形式にする。
  const roleLabel = useCallback(
    (message: IdentifiableMessage): string => {
      if (message.role === 'assistant') {
        const modelId = message.metadata?.modelId
        return modelId ? `Assistant – ${modelId}` : 'Assistant'
      }
      return userName ? `User – ${userName}` : 'User'
    },
    [userName]
  )

  const handleDeleteMessage = (index: number) => {
    // メッセージの配列のコピーを作成
    const updatedMessages = [...messages]

    // メッセージを削除
    updatedMessages.splice(index, 1)

    // 更新されたメッセージの配列を設定
    setMessages(updatedMessages)

    // チャット履歴が有効な場合は、対応するメッセージを削除
    if (currentSessionId) {
      deleteMessage(currentSessionId, index)
    }
  }

  // Ignore Settings Modal の状態管理
  const [showIgnoreSettingsModal, setShowIgnoreSettingsModal] = useState(false)
  const handleOpenIgnoreFileModal = () => setShowIgnoreSettingsModal(true)
  const handleCloseIgnoreFileModal = () => setShowIgnoreSettingsModal(false)

  const {
    show: showSystemPromptModal,
    handleClose: handleCloseSystemPromptModal,
    handleOpen: handleOpenSystemPromptModal,
    SystemPromptModal
  } = useSystemPromptModal()

  const {
    show: showTokenAnalyticsModal,
    handleClose: handleCloseTokenAnalyticsModal,
    handleOpen: handleOpenTokenAnalyticsModal,
    TokenAnalyticsModal
  } = useTokenAnalyticsModal()

  // 会話全体の実行コストを計算（ヘッダー表示用）。Help チャットは軽量モデルで動くので、
  // グローバルの選択モデルではなく実際に使われているモデルで計算する。
  const runningCost = useMemo(
    () => calculateAnalytics(messages, activeModelId || '').costAnalysis.totalCost,
    [messages, activeModelId]
  )

  const {
    show: showTodoModal,
    handleClose: handleCloseTodoModal,
    handleOpen: handleOpenTodoModal,
    flash: todoFlash,
    TodoModal
  } = useTodoModal(messages, currentSessionId, loading)

  // Flash the TODO header icon in the changed task's status color on each update
  const [todoFlashColor, setTodoFlashColor] = useState<string | null>(null)
  useEffect(() => {
    if (!todoFlash) return undefined
    const statusColors: Record<string, string> = {
      pending: 'text-warning',
      in_progress: 'text-accent',
      completed: 'text-success',
      cancelled: 'text-danger'
    }
    setTodoFlashColor(statusColors[todoFlash.status] ?? null)
    // Animation runs 0.5s x 2; clear slightly after so the class re-applies next time
    const timer = setTimeout(() => setTodoFlashColor(null), 1100)
    return () => clearTimeout(timer)
  }, [todoFlash?.nonce])

  const {
    show: showToolSettingModal,
    handleClose: handleCloseToolSettingModal,
    handleOpen: handleOpenToolSettingModal,
    ToolSettingModal
  } = useToolSettingModal()

  // このチャットの Docker サンドボックス状態（存在しなければツールバーに何も出ない）
  const {
    status: sandboxStatus,
    isBusy: isSandboxBusy,
    stop: stopSandbox,
    start: startSandbox,
    remove: removeSandbox,
    openFolder: openSandboxFolder
  } = useChatSandbox(currentSessionId)

  // このチャットに添付されたファイル（プロジェクト内の attachments/<チャット> フォルダ）
  const attachments = useChatAttachments(currentSessionId)

  const handleStopSandbox = useCallback(async () => {
    try {
      await stopSandbox()
      toast.success(t('dockerSandbox.toast.stopped'))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
  }, [stopSandbox, t])

  const handleStartSandbox = useCallback(async () => {
    try {
      await startSandbox()
      toast.success(t('dockerSandbox.toast.started'))
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
  }, [startSandbox, t])

  const handleRemoveSandbox = useCallback(
    async (deleteData: boolean) => {
      try {
        await removeSandbox(deleteData)
        toast.success(
          deleteData ? t('dockerSandbox.toast.removedWithData') : t('dockerSandbox.toast.removed')
        )
      } catch (error) {
        toast.error(error instanceof Error ? error.message : String(error))
      }
    },
    [removeSandbox, t]
  )

  const handleOpenSandboxFolder = useCallback(async () => {
    try {
      await openSandboxFolder()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error))
    }
  }, [openSandboxFolder])

  // クリアハンドラをuseCallbackでメモ化
  const handleClearChat = useCallback(() => {
    if (window.confirm(t('confirmClearChat'))) {
      clearChat()
    }
  }, [clearChat, t])

  // エクスポート用のタイトルを確定する（未生成のデフォルトタイトルなら生成する）
  const resolveExportTitle = useCallback(async (): Promise<string> => {
    if (!currentSessionId) return 'Chat Export'

    let session = getSession(currentSessionId)
    if (session && session.title.startsWith('Chat ')) {
      const newTitle = await generateSessionTitle(session, getLightModelId(), t)
      if (newTitle) {
        await updateSessionTitle(currentSessionId, newTitle)
        session = getSession(currentSessionId)
      }
    }
    return session?.title?.trim() || 'Chat Export'
  }, [currentSessionId, getSession, updateSessionTitle, getLightModelId, t])

  // チャットを Markdown にエクスポートするハンドラ
  const handleExportChat = useCallback(async () => {
    if (!currentSessionId || messages.length === 0 || isExporting) return

    setIsExporting(true)
    const loadingToast = toast.loading(t('Exporting chat...'))
    try {
      const title = await resolveExportTitle()

      // Markdown を構築し、図表・画像を PNG にラスタライズする
      // 見出しは docx 版と同じ「Assistant – <modelId>」「User – <userName>」＋アバター画像
      const { markdown, images } = await buildChatMarkdown(title, messages, {
        rasterizeDrawio: (xml) =>
          drawioRasterizerRef.current?.rasterize(xml) ?? Promise.resolve(null),
        roleLabel,
        avatarDataUrl: (message) => avatarDataUrl(message, { userEmoji, resolveModel })
      })

      // ディスクに書き込む
      const result = await window.file.exportChatMarkdown({ title, markdown, images })

      toast.dismiss(loadingToast)
      if (result.success) {
        toast.success(`${t('Chat exported to')} ${result.directory}`)
      } else {
        toast.error(`${t('Failed to export chat')}: ${result.error ?? ''}`)
      }
    } catch (error) {
      console.error('Failed to export chat:', error)
      toast.dismiss(loadingToast)
      toast.error(t('Failed to export chat'))
    } finally {
      setIsExporting(false)
    }
  }, [
    currentSessionId,
    messages,
    isExporting,
    resolveExportTitle,
    roleLabel,
    userEmoji,
    resolveModel,
    t
  ])

  // チャットを Word (.docx) にエクスポートするハンドラ（Markdown 版と同じ内容ルール）
  const handleExportWord = useCallback(async () => {
    if (!currentSessionId || messages.length === 0 || isExportingWord) return

    setIsExportingWord(true)
    const loadingToast = toast.loading(t('Exporting chat...'))
    try {
      const title = await resolveExportTitle()

      // リッチテキスト HTML を構築する（見出し・アバター・図表を埋め込む）
      const { html } = await buildChatHtml(title, messages, {
        rasterizeDrawio: (xml) =>
          drawioRasterizerRef.current?.rasterize(xml) ?? Promise.resolve(null),
        roleLabel,
        avatar: { userEmoji, resolveModel }
      })

      // ディスクに書き込む
      const result = await window.file.exportChatDocx({ title, html })

      toast.dismiss(loadingToast)
      if (result.success) {
        toast.success(`${t('Chat exported to')} ${result.directory}`)
      } else {
        toast.error(`${t('Failed to export chat')}: ${result.error ?? ''}`)
      }
    } catch (error) {
      console.error('Failed to export chat to Word:', error)
      toast.dismiss(loadingToast)
      toast.error(t('Failed to export chat'))
    } finally {
      setIsExportingWord(false)
    }
  }, [
    currentSessionId,
    messages,
    isExportingWord,
    resolveExportTitle,
    roleLabel,
    userEmoji,
    resolveModel,
    t
  ])

  // チャットを PDF にエクスポートするハンドラ（Markdown 版と同じ内容ルール）
  const handleExportPdf = useCallback(async () => {
    if (!currentSessionId || messages.length === 0 || isExportingPdf) return

    setIsExportingPdf(true)
    const loadingToast = toast.loading(t('Exporting chat...'))
    try {
      const title = await resolveExportTitle()

      // 印刷用の HTML を構築する（画像はすべてインライン化される）
      const { html } = await buildChatPdfHtml(title, messages, {
        rasterizeDrawio: (xml) =>
          drawioRasterizerRef.current?.rasterize(xml) ?? Promise.resolve(null),
        roleLabel,
        avatar: { userEmoji, resolveModel }
      })

      // main プロセスで PDF に印刷して書き込む
      const result = await window.file.exportChatPdf({ title, html })

      toast.dismiss(loadingToast)
      if (result.success) {
        toast.success(`${t('Chat exported to')} ${result.directory}`)
      } else {
        toast.error(`${t('Failed to export chat')}: ${result.error ?? ''}`)
      }
    } catch (error) {
      console.error('Failed to export chat to PDF:', error)
      toast.dismiss(loadingToast)
      toast.error(t('Failed to export chat'))
    } finally {
      setIsExportingPdf(false)
    }
  }, [
    currentSessionId,
    messages,
    isExportingPdf,
    resolveExportTitle,
    roleLabel,
    userEmoji,
    resolveModel,
    t
  ])

  // シナリオ選択ハンドラ
  const handleSelectScenario = useCallback((scenario: string) => {
    // テキストエリアに選択したシナリオを設定
    if (inputFormRef.current) {
      inputFormRef.current.setInputText(scenario)
    }
  }, [])

  // 履歴は開いた状態で始める。ChatPage は画面遷移でアンマウントされるので、
  // Chat を開き直すたびに履歴が開く。
  const [isHistoryOpen, setIsHistoryOpen] = useState(true)
  // サンドボックスパネルは既定で閉じる。履歴と違い、必要なときだけ開くもの。
  const [isSandboxPanelOpen, setIsSandboxPanelOpen] = useState(false)
  const DEFAULT_TEXTAREA_HEIGHT = 72 // Default height (3 lines * 24px)

  const [textareaHeight, setTextareaHeight] = useState(DEFAULT_TEXTAREA_HEIGHT)

  // Streaming auto-scroll: follows tool-use output, then stops when the answer's first line
  // reaches the top. The signature changes on every streamed update so the hook re-evaluates.
  const messageAreaRef = useRef<HTMLDivElement>(null)
  const lastMessage = messages[messages.length - 1]
  const lastTextBlock = lastMessage?.content?.find((c) => 'text' in c) as
    | { text?: string }
    | undefined
  const scrollSignature = `${messages.length}:${lastMessage?.content?.length ?? 0}:${lastTextBlock?.text?.length ?? 0}`
  useStreamingAutoScroll(messageAreaRef, { signature: scrollSignature, loading })

  const handleSessionSelect = (sessionId: string) => {
    setCurrentSessionId(sessionId)
  }

  // URLパラメータからプロンプトとエージェントを自動設定
  useEffect(() => {
    const searchParams = new URLSearchParams(location.search)
    const promptFromUrl = searchParams.get('prompt')
    const agentFromUrl = searchParams.get('agent')

    if (promptFromUrl && inputFormRef.current) {
      // プロンプトをテキストエリアに設定
      inputFormRef.current.setInputText(decodeURIComponent(promptFromUrl))

      // 指定されたエージェントに切り替え
      if (agentFromUrl && agents.find((a) => a.id === agentFromUrl)) {
        setSelectedAgentId(agentFromUrl)
      }

      // URLパラメータをクリア（同じリンクを再度クリックした時の対応）
      const newUrl = window.location.pathname + window.location.hash
      window.history.replaceState({}, '', newUrl)
    }
  }, [location.search, agents, setSelectedAgentId])

  // Opening the Help chat from the sidebar (`/chat?help=1`).
  //
  // Reuse comes first so repeated clicks continue the same conversation instead of leaving a
  // trail of half-empty Help chats, each with its own copy of the guide on disk.
  const helpOpenInFlight = useRef(false)
  useEffect(() => {
    if (!new URLSearchParams(location.search).has('help')) return
    if (helpOpenInFlight.current) return
    helpOpenInFlight.current = true

    const openHelpChat = async () => {
      try {
        let sessionId = currentSessionId
        const alreadyOnHelp = !!sessionId && getSession(sessionId)?.agentId === HELP_AGENT_ID

        if (!alreadyOnHelp) {
          // Session metadata hides chats with no messages, so an untouched Help chat can be
          // missing here. The `alreadyOnHelp` check above covers the case that matters.
          const previous = sessions
            .filter((session) => session.agentId === HELP_AGENT_ID)
            .sort((a, b) => b.updatedAt - a.updatedAt)[0]

          if (previous) {
            sessionId = previous.id
          } else {
            sessionId = await createSession(
              HELP_AGENT_ID,
              getLightModelId(),
              HELP_AGENT_SYSTEM_PROMPT
            )
            // Titled before the guide is attached so the attachments folder is named from the
            // real title on creation. A title that isn't "Chat …" is also never auto-renamed.
            await updateSessionTitle(sessionId, HELP_CHAT_TITLE)
          }
        }

        if (!sessionId) return

        const result = await window.api.help.prepareUserGuide(sessionId)
        if (result.attached) {
          setHelpGuideText(undefined)
        } else {
          setHelpGuideText(result.text)
          toast(t('help.guideInPromptNotice'))
        }

        setIsHelpSession(true)
        if (sessionId !== currentSessionId) setCurrentSessionId(sessionId)
        if (result.attached) await attachments.refresh()
      } catch (error) {
        toast.error(
          t('help.guideFailed', {
            error: error instanceof Error ? error.message : String(error)
          })
        )
      } finally {
        // Cleared only after the param is gone, so the effect cannot re-enter on the same click.
        navigate('/chat', { replace: true })
        helpOpenInFlight.current = false
      }
    }

    void openHelpChat()
  }, [location.search])

  return (
    <HelpSessionProvider
      value={{
        isHelpSession,
        // The seeded agent may not be in the store yet on the first launch after an update.
        agentName: helpAgent?.name ?? HELP_CHAT_TITLE,
        modelId: activeModelId
      }}
    >
      {/* Hidden DrawIO embed used to rasterize diagrams during markdown export */}
      <DrawioRasterizer ref={drawioRasterizerRef} />
      <div className="flex px-4 py-3 h-screen">
        {/* チャット履歴サイドパネル - 全高。ヘッダーより上（ウィンドウ最上部）から始まる */}
        <div
          className={`bg-canvas flex-shrink-0 transition-all duration-300 ease-in-out ${
            isHistoryOpen ? 'w-96' : 'w-0'
          } overflow-y-auto overflow-x-hidden`}
        >
          {/* Fixed-width inner keeps content from reflowing while the panel
              width animates, so the chat column slides smoothly. */}
          <div className="w-96">
            <ChatHistory
              onSessionSelect={handleSessionSelect}
              currentSessionId={currentSessionId}
            />
          </div>
        </div>

        {/* チャット履歴トグルバー */}
        <div className="flex items-center">
          <Tooltip
            content={t(isHistoryOpen ? 'Hide chat history' : 'Show chat history')}
            placement="right"
            animation="duration-500"
          >
            <div
              onClick={() => setIsHistoryOpen(!isHistoryOpen)}
              className="w-4 h-16 bg-raised cursor-pointer flex items-center justify-center transition-colors duration-200 rounded-container m-2"
            >
              <FiChevronRight
                className={`w-4 h-4 text-accent hover:text-accent-strong transition-transform duration-200 ${
                  isHistoryOpen ? 'rotate-180' : ''
                }`}
              />
            </div>
          </Tooltip>
        </div>

        {/* チャット列（ヘッダー + メインコンテンツ） - フレックス成長。履歴表示時に右へスライド */}
        <div className="flex-1 flex flex-col min-w-0">
          {/* ヘッダー - 固定 */}
          <div className="flex justify-end items-center">
            <div className="flex items-center gap-2">
              {/* Only show TODO icon when there are messages */}
              {messages.length > 0 && (
                <FaListCheck
                  key={todoFlash?.nonce ?? 0}
                  className={`cursor-pointer ${
                    todoFlashColor
                      ? `${todoFlashColor} animate-todo-flash`
                      : 'text-ink-faint hover:text-ink'
                  }`}
                  onClick={() => handleOpenTodoModal()}
                  title={t('View TODO List')}
                  size={16}
                />
              )}
              {messages.length > 0 && (
                <span
                  className="text-xs text-ink-faint font-medium cursor-pointer hover:text-ink"
                  onClick={handleOpenTokenAnalyticsModal}
                  title={t('Conversation Cost')}
                >
                  {runningCost.toLocaleString('en-US', {
                    style: 'currency',
                    currency: 'USD',
                    minimumFractionDigits: 2,
                    maximumFractionDigits: 2
                  })}
                </span>
              )}
              <FiBarChart2
                className="text-ink-faint hover:text-ink cursor-pointer"
                onClick={handleOpenTokenAnalyticsModal}
                title={t('View Token Analytics')}
                size={16}
              />
              <span
                className="text-xs text-ink-faint font-thin cursor-pointer hover:text-ink"
                onClick={handleOpenSystemPromptModal}
              >
                SYSTEM_PROMPT
              </span>
            </div>
          </div>

          {/* Modals */}
          <HostCommandApprovalModal />
          <SystemPromptModal
            isOpen={showSystemPromptModal}
            onClose={handleCloseSystemPromptModal}
            systemPrompt={activeSystemPrompt}
          />
          <TokenAnalyticsModal
            isOpen={showTokenAnalyticsModal}
            onClose={handleCloseTokenAnalyticsModal}
            messages={messages}
            modelId={activeModelId || ''}
          />
          <TodoModal isOpen={showTodoModal} onClose={handleCloseTodoModal} />
          <ToolSettingModal isOpen={showToolSettingModal} onClose={handleCloseToolSettingModal} />
          <IgnoreSettingsModal
            isOpen={showIgnoreSettingsModal}
            onClose={handleCloseIgnoreFileModal}
            projectPath={projectPath}
          />

          {/* メインコンテンツエリア - フレックス成長 */}
          {/* pb-52 (13rem) reserves space for the fixed input form. */}
          <div className="flex flex-col flex-1 min-h-0 pb-52">
            {/* Adjusts the bottom padding of the message area based on the height of the text area */}
            <div
              ref={messageAreaRef}
              className="flex-1 overflow-y-auto mb-2"
              style={{
                // Additional padding is applied only when the text area grows larger
                // This ensures that as the text area gets taller, the bottom padding of the message area increases by the same amount
                paddingBottom: `${textareaHeight - DEFAULT_TEXTAREA_HEIGHT * 2}px`
              }}
            >
              {messages.length === 0 ? (
                <div className="flex flex-col pt-12 h-full w-full justify-center items-center content-center gap-1">
                  <div className="flex flex-row gap-3 items-center mb-2">
                    <div className="bg-icon rounded-container p-1.5 flex items-center justify-center">
                      <div className="h-4 w-4">
                        <img
                          src={AILogo}
                          className="h-full w-full object-contain"
                          alt="assistant"
                        />
                      </div>
                    </div>
                    <h1 className="text-title text-ink">
                      {isHelpSession ? t('help.chatTitle') : 'Agent Chat'}
                    </h1>
                  </div>
                  <div className="text-ink-faint">
                    {t((isHelpSession ? helpAgent?.description : currentAgent?.description) ?? '')}
                  </div>
                  {(isHelpSession ? helpAgent : currentAgent) && (
                    <ExampleScenarios
                      scenarios={currentScenarios}
                      onSelectScenario={handleSelectScenario}
                    />
                  )}
                </div>
              ) : (
                <div className="py-8">
                  <MessageList
                    messages={messages}
                    loading={loading}
                    reasoning={reasoning}
                    waitingForResponse={waitingForResponse}
                    timeoutCountdown={timeoutCountdown}
                    heartbeatCount={heartbeatCount}
                    deleteMessage={handleDeleteMessage}
                  />
                </div>
              )}
            </div>

            {/* 入力フォーム - 固定 */}
            <div className="mt-2 border-subtle bg-surface">
              <InputFormContainer
                ref={inputFormRef}
                loading={loading}
                projectPath={projectPath}
                sendMsgKey={sendMsgKey}
                onSubmit={onSubmit}
                onOpenToolSettings={handleOpenToolSettingModal}
                onSelectDirectory={selectDirectory}
                onOpenIgnoreModal={handleOpenIgnoreFileModal}
                onClearChat={handleClearChat}
                onExportChat={handleExportChat}
                isExporting={isExporting}
                onExportWord={handleExportWord}
                isExportingWord={isExportingWord}
                onExportPdf={handleExportPdf}
                isExportingPdf={isExportingPdf}
                onStopGeneration={stopGeneration}
                hasMessages={messages.length > 0}
                onHeightChange={setTextareaHeight}
                isHistoryOpen={isHistoryOpen}
                attachments={{
                  files: attachments.files,
                  directory: attachments.directory,
                  totalSize: attachments.totalSize,
                  isBusy: attachments.isBusy,
                  onAdd: attachments.addFromPicker,
                  onRemove: attachments.remove,
                  onOpenFolder: attachments.openFolder,
                  onRefresh: attachments.refresh,
                  onAddFiles: attachments.addFiles
                }}
                sandbox={{
                  status: sandboxStatus,
                  isBusy: isSandboxBusy,
                  onStop: handleStopSandbox,
                  onStart: handleStartSandbox,
                  onRemove: handleRemoveSandbox,
                  onOpenFolder: handleOpenSandboxFolder,
                  onOpenPanel: () => setIsSandboxPanelOpen(true)
                }}
              />
            </div>
          </div>
        </div>

        {/* サンドボックスパネルのトグルバー - サンドボックスがあるチャットにのみ出す */}
        {sandboxStatus.exists && (
          // pb-52 matches the message area's reservation for the fixed input box, so the
          // strip stays centred against the panel rather than the whole window.
          <div className="flex items-center pb-52">
            <Tooltip
              content={t(
                isSandboxPanelOpen ? 'dockerSandbox.panel.hide' : 'dockerSandbox.panel.show'
              )}
              placement="left"
              animation="duration-500"
            >
              <div
                onClick={() => setIsSandboxPanelOpen(!isSandboxPanelOpen)}
                className="w-4 h-16 bg-raised cursor-pointer flex items-center justify-center transition-colors duration-200 rounded-container m-2"
              >
                <FiChevronRight
                  className={`w-4 h-4 text-accent hover:text-accent-strong transition-transform duration-200 ${
                    isSandboxPanelOpen ? '' : 'rotate-180'
                  }`}
                />
              </div>
            </Tooltip>
          </div>
        )}

        {/* サンドボックスパネル - 幅アニメーション。内側は固定幅なので
            アニメーション中にチャット列が詰まって見えない */}
        {/* pb-52 keeps the panel clear of the fixed input box and its icons, the same
            reservation the message list makes, so the input can stay full width. */}
        <div
          className={`bg-canvas flex-shrink-0 transition-all duration-300 ease-in-out overflow-hidden pb-52 ${
            isSandboxPanelOpen && sandboxStatus.exists ? 'w-[40rem]' : 'w-0'
          }`}
        >
          <SandboxPanel
            sessionId={currentSessionId}
            status={sandboxStatus}
            isBusy={isSandboxBusy}
            isOpen={isSandboxPanelOpen && sandboxStatus.exists}
            onStart={handleStartSandbox}
            onStop={handleStopSandbox}
            onRemove={handleRemoveSandbox}
            onOpenFolder={handleOpenSandboxFolder}
          />
        </div>
      </div>
    </HelpSessionProvider>
  )
}

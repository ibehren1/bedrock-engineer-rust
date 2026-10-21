import { useEffect, useState, useCallback, useRef, useMemo } from 'react'
import AILogo from '../../assets/images/icons/bedrock-color.png'
import { MessageList } from './components/MessageList'
import InputFormContainer, { InputFormContainerRef } from './components/InputFormContainer'
import { ExampleScenarios } from './components/ExampleScenarios'
import { useAgentChat } from './hooks/useAgentChat'
import useSetting from '@renderer/hooks/useSetting'
import { IgnoreSettingsModal } from '@renderer/components/IgnoreSettingsModal'
import { useToolSettingModal } from './modals/useToolSettingModal'
import { FiBarChart2, FiFileText, FiFolder, FiMessageSquare } from 'react-icons/fi'
import { TbTerminal2 } from 'react-icons/tb'
import { FaListCheck } from 'react-icons/fa6'
import { FaDocker } from 'react-icons/fa'
import { useTranslation } from 'react-i18next'
import { AttachedImage } from './components/InputForm/TextArea'
import { HistorySidebar } from './components/HistorySidebar'
import { SystemPromptView } from './modals/useSystemPromptModal'
import { TokenAnalyticsView, calculateAnalytics } from './components/TokenAnalyticsView'
import { TodoListView, useTodoList } from './components/TodoListView'
import { HostCommandApprovalModal } from './modals/HostCommandApprovalModal'
import { useChatSandbox } from './hooks/useChatSandbox'
import { FilesTab, type FilesOpenRequest } from './components/RightPanel/FilesTab'
import { SandboxPanel } from './components/SandboxPanel'
import { LocalTerminalPanel } from './components/Terminal/LocalTerminalPanel'
import { useLocalTerminals } from './components/Terminal/useLocalTerminals'
import { ChatTabStrip, type ChatTabItem } from './components/ChatTabs/ChatTabStrip'
import {
  effectiveTab,
  TabMemory,
  visibleTabs,
  type ChatTab,
  type ChatTabConditions
} from './components/ChatTabs/chatTabs'
import {
  FilesPanelContext,
  type FilesPanelContextValue
} from '@renderer/contexts/FilesPanelContext'
import { forgetMissing } from '@renderer/lib/workspaceFiles/pathResolver'
import { subscribeTouchedFiles, useTouchedFiles } from '@renderer/lib/workspaceFiles/touchedFiles'
import { usePanelAttention } from './components/RightPanel/usePanelAttention'
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
import { useMemoryItems } from '@renderer/contexts/MemoryContext'
import { withMemoryPrompt } from '@common/agents/memoryPrompt'

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

  const {
    deleteMessage,
    getSession,
    updateSessionTitle,
    createSession,
    sessions,
    setUseMemoryItems
  } = useChatHistory()
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
  // Memory items go into normal chats whose "Use Memory Items" box is checked (per session,
  // default on). The Help chat has no tools and its own prompt, so it never gets them.
  const { items: memoryItems, enabledCount: enabledMemoryCount } = useMemoryItems()
  const [useMemory, setUseMemory] = useState(true)
  const chatSystemPrompt = useMemo(
    () => withMemoryPrompt(systemPrompt, memoryItems, useMemory),
    [systemPrompt, memoryItems, useMemory]
  )

  const activeSystemPrompt = isHelpSession
    ? helpGuideText
      ? `${HELP_AGENT_SYSTEM_PROMPT}\n<user_guide>\n${helpGuideText}\n</user_guide>\n`
      : HELP_AGENT_SYSTEM_PROMPT
    : chatSystemPrompt

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

  // The tab showing in each chat (see TabMemory); chats open on Chat.
  const tabMemory = useRef(new TabMemory())
  const [chosenTab, setChosenTab] = useState<ChatTab>('chat')
  useEffect(() => {
    setChosenTab(tabMemory.current.get(currentSessionId))
  }, [currentSessionId])
  const selectTab = useCallback(
    (tab: ChatTab) => {
      tabMemory.current.set(currentSessionId, tab)
      setChosenTab(tab)
    },
    [currentSessionId]
  )

  // 送信ハンドラをuseCallbackでメモ化。送信すると Chat タブに戻り、返答が見えるようにする。
  const onSubmit = useCallback(
    (input: string, images: AttachedImage[]) => {
      selectTab('chat')
      handleSubmit(input, images)
    },
    [handleSubmit, selectTab]
  )

  // 表示中のセッションが Help かどうかを追随させる。
  useEffect(() => {
    const agentId = currentSessionId ? getSession(currentSessionId)?.agentId : undefined
    setIsHelpSession(agentId === HELP_AGENT_ID)
  }, [currentSessionId, getSession])

  // Load the chat's "Use Memory Items" setting; sessions without one (new chats) default to on.
  useEffect(() => {
    const session = currentSessionId ? getSession(currentSessionId) : null
    setUseMemory(session?.useMemoryItems ?? true)
  }, [currentSessionId, getSession])

  const handleUseMemoryChange = useCallback(
    (value: boolean) => {
      if (!currentSessionId) return
      setUseMemory(value)
      setUseMemoryItems(currentSessionId, value).catch((error) => {
        console.error('Failed to save the Use Memory Items setting:', error)
        toast.error(error instanceof Error ? error.message : String(error))
      })
    },
    [currentSessionId, setUseMemoryItems]
  )

  // DrawIO diagrams render in an iframe, so a live component is needed to rasterize them.
  const drawioRasterizerRef = useRef<DrawioRasterizerRef>(null)
  const [isExporting, setIsExporting] = useState(false)
  const [isExportingWord, setIsExportingWord] = useState(false)
  const [isExportingPdf, setIsExportingPdf] = useState(false)

  // モデルIDから表示情報を解決する（チャットのアバターと同じロジック）。
  const resolveModel = useCallback(
    (modelId?: string) =>
      modelId
        ? (availableModels.find((m) => m.modelId === modelId) ??
          allModels.find((m) => m.modelId === modelId))
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

  // 会話全体の実行コストを計算（Cost タブのラベル用）。Help チャットは軽量モデルで動くので、
  // グローバルの選択モデルではなく実際に使われているモデルで計算する。
  const runningCost = useMemo(
    () => calculateAnalytics(messages, activeModelId || '').costAnalysis.totalCost,
    [messages, activeModelId]
  )

  const {
    todoList,
    loading: todoLoading,
    flash: todoFlash,
    handleRefresh: refreshTodoList
  } = useTodoList(messages, currentSessionId, loading, chosenTab === 'todo')

  // Flash the Todo tab's icon in the changed task's status color on each update
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
  // Chat を開き直すたびに履歴が開く。閉じると細いレールになる。
  const [isHistoryOpen, setIsHistoryOpen] = useState(true)

  // Shells on this computer, per chat (the Terminal tab).
  const localTerminals = useLocalTerminals(currentSessionId)

  const todoItems = todoList?.items ?? []
  const tabConditions: ChatTabConditions = {
    hasProject: !!projectPath,
    hasSandbox: sandboxStatus.exists,
    hasTerminal: localTerminals.terminals.length > 0,
    hasTodo: todoItems.length > 0,
    hasMessages: messages.length > 0
  }
  // The chosen tab can stop being listed (the sandbox was removed, say): show Chat until it is.
  const activeTab = effectiveTab(chosenTab, tabConditions)

  const [filesOpenRequest, setFilesOpenRequest] = useState<FilesOpenRequest>()
  // Files and Sandbox mount on first view, then stay mounted so the tree, editor and an open
  // shell survive tab switches.
  const [filesMounted, setFilesMounted] = useState(false)
  useEffect(() => {
    if (activeTab === 'files') setFilesMounted(true)
  }, [activeTab])

  // The Files and Sandbox tabs pulse (and keep a marker) when there is something new in them.
  const touchedFiles = useTouchedFiles(currentSessionId)
  const filesAttention = usePanelAttention(
    {
      sessionId: currentSessionId,
      visible: tabConditions.hasProject,
      hasSandbox: false,
      touchedCount: touchedFiles.length
    },
    activeTab === 'files'
  )
  const sandboxAttention = usePanelAttention(
    {
      sessionId: currentSessionId,
      visible: tabConditions.hasSandbox,
      hasSandbox: tabConditions.hasSandbox,
      touchedCount: 0
    },
    activeTab === 'sandbox'
  )

  const toggleFilesTab = useCallback(() => {
    selectTab(activeTab === 'files' ? 'chat' : 'files')
  }, [activeTab, selectTab])

  // The Terminal button: go to the chat's shells, starting one when there are none (or when the
  // tab is already showing, which asks for another).
  const { openTerminal } = localTerminals
  const hasTerminal = tabConditions.hasTerminal
  const openTerminalTab = useCallback(async () => {
    try {
      if (!hasTerminal || activeTab === 'terminal') await openTerminal()
      selectTab('terminal')
    } catch (error) {
      toast.error(
        t('filesPanel.terminalOpenFailed', {
          error: error instanceof Error ? error.message : String(error)
        })
      )
    }
  }, [hasTerminal, activeTab, openTerminal, selectTab, t])

  // File paths in chat messages open here. Misses are looked up again when files change, since
  // the agent often names a file before writing it.
  const [filesVersion, setFilesVersion] = useState(0)
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined
    const bump = () => {
      if (timer) return
      timer = setTimeout(() => {
        timer = undefined
        forgetMissing()
        setFilesVersion((v) => v + 1)
      }, 400)
    }
    const unlistenChanges = window.workspaceFiles.onChanged(bump)
    const unsubscribeTouched = subscribeTouchedFiles(bump)
    return () => {
      if (timer) clearTimeout(timer)
      unlistenChanges()
      unsubscribeTouched()
    }
  }, [])

  const openFileInPanel = useCallback(
    (path: string, kind?: 'file' | 'dir') => {
      setFilesOpenRequest((prev) => ({ path, kind, nonce: (prev?.nonce ?? 0) + 1 }))
      selectTab('files')
    },
    [selectTab]
  )

  const filesPanelContext = useMemo<FilesPanelContextValue | null>(
    () => (projectPath ? { projectPath, openFile: openFileInPanel, filesVersion } : null),
    [projectPath, openFileInPanel, filesVersion]
  )
  // Streaming auto-scroll: follows tool-use output, then stops when the answer's first line
  // reaches the top. The signature changes on every streamed update so the hook re-evaluates.
  const messageAreaRef = useRef<HTMLDivElement>(null)
  const lastMessage = messages[messages.length - 1]
  const lastTextBlock = lastMessage?.content?.find((c) => 'text' in c) as
    { text?: string } | undefined
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

  const paneClass = (tab: ChatTab) =>
    `absolute inset-0 ${activeTab === tab ? '' : 'invisible pointer-events-none'}`

  const todoDone = todoItems.filter((item) => item.status === 'completed').length
  const formattedCost = runningCost.toLocaleString('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2
  })
  // Restarted by its key, so each new piece of attention pings again; none with reduced motion.
  const attentionPing = (pulseKey: number) =>
    pulseKey > 0 && (
      <span
        key={pulseKey}
        aria-hidden
        className="pointer-events-none absolute inset-0 rounded-full bg-touched opacity-0 motion-safe:animate-attention-ping"
      />
    )

  const tabItem = (tab: ChatTab): ChatTabItem => {
    switch (tab) {
      case 'chat':
        return {
          key: tab,
          label: t('filesPanel.tabChat'),
          icon: <FiMessageSquare className="size-3.5" />
        }
      case 'files':
        return {
          key: tab,
          label: t('filesPanel.tabFiles'),
          icon: <FiFolder className="size-3.5" />,
          adornment: filesAttention.badge && touchedFiles.length > 0 && (
            <span className="relative" title={t('filesPanel.newFiles')}>
              {attentionPing(filesAttention.pulseKey)}
              <span className="relative block min-w-4 h-4 px-1 rounded-full bg-touched text-white text-micro leading-4 text-center">
                {touchedFiles.length}
              </span>
            </span>
          )
        }
      case 'sandbox':
        return {
          key: tab,
          label: t('filesPanel.tabSandbox'),
          icon: <FaDocker className="size-3.5 text-[#2496ED]" />,
          adornment: sandboxAttention.badge && (
            <span className="relative size-2" title={t('filesPanel.sandboxActivity')}>
              {attentionPing(sandboxAttention.pulseKey)}
              <span className="relative block size-2 rounded-full bg-touched" />
            </span>
          )
        }
      case 'todo':
        return {
          key: tab,
          label: t('filesPanel.tabTodo'),
          // Flashes in the changed task's status colour on each update.
          icon: (
            <FaListCheck
              key={todoFlash?.nonce ?? 0}
              className={`size-3.5 ${todoFlashColor ? `${todoFlashColor} animate-todo-flash` : ''}`}
            />
          ),
          adornment: (
            <span className="text-ink-faint tabular-nums">
              {todoDone}/{todoItems.length}
            </span>
          ),
          title: t('filesPanel.todoProgress', { done: todoDone, total: todoItems.length })
        }
      case 'cost':
        return {
          key: tab,
          label: t('filesPanel.tabCost'),
          icon: <FiBarChart2 className="size-3.5" />,
          adornment: <span className="text-ink-faint tabular-nums">{formattedCost}</span>,
          title: t('filesPanel.costTooltip')
        }
      case 'terminal':
        return {
          key: tab,
          label: t('filesPanel.tabTerminal'),
          icon: <TbTerminal2 className="size-3.5" />,
          adornment:
            localTerminals.terminals.length > 1 ? (
              <span className="text-ink-faint tabular-nums">{localTerminals.terminals.length}</span>
            ) : undefined
        }
      case 'systemPrompt':
        return {
          key: tab,
          label: t('filesPanel.tabSystemPrompt'),
          icon: <FiFileText className="size-3.5" />
        }
    }
  }
  const tabItems = visibleTabs(tabConditions).map(tabItem)

  return (
    <HelpSessionProvider
      value={{
        isHelpSession,
        // The seeded agent may not be in the store yet on the first launch after an update.
        agentName: helpAgent?.name ?? HELP_CHAT_TITLE,
        modelId: activeModelId
      }}
    >
      <FilesPanelContext.Provider value={filesPanelContext}>
        {/* Hidden DrawIO embed used to rasterize diagrams during markdown export */}
        <DrawioRasterizer ref={drawioRasterizerRef} />
        <div className="flex h-screen">
          <HistorySidebar
            open={isHistoryOpen}
            onOpenChange={setIsHistoryOpen}
            currentSessionId={currentSessionId}
            onSessionSelect={handleSessionSelect}
            onNewChat={handleClearChat}
          />

          {/* チャット列: タブ + タブの中身 + 入力フォーム（どのタブでも表示） */}
          <div className="flex-1 flex flex-col min-w-0 px-4 pt-1 pb-3">
            <ChatTabStrip items={tabItems} active={activeTab} onChange={selectTab} />

            {/* Modals */}
            <HostCommandApprovalModal />
            <ToolSettingModal isOpen={showToolSettingModal} onClose={handleCloseToolSettingModal} />
            <IgnoreSettingsModal
              isOpen={showIgnoreSettingsModal}
              onClose={handleCloseIgnoreFileModal}
              projectPath={projectPath}
            />

            {/* Tab bodies are stacked; the hidden ones keep their layout (visibility, not
                display) so the chat keeps its scroll position and a shell its size. */}
            <div className="relative flex-1 min-h-0">
              <div ref={messageAreaRef} className={`${paneClass('chat')} overflow-y-auto`}>
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
                      {t(
                        (isHelpSession ? helpAgent?.description : currentAgent?.description) ?? ''
                      )}
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

              {tabConditions.hasProject && filesMounted && (
                <div className={paneClass('files')}>
                  <FilesTab
                    projectPath={projectPath ?? ''}
                    sessionId={currentSessionId}
                    active={activeTab === 'files'}
                    openRequest={filesOpenRequest}
                  />
                </div>
              )}

              {tabConditions.hasSandbox && (
                <div className={paneClass('sandbox')}>
                  <SandboxPanel
                    embedded
                    sessionId={currentSessionId}
                    status={sandboxStatus}
                    isBusy={isSandboxBusy}
                    isOpen={activeTab === 'sandbox'}
                    onStart={handleStartSandbox}
                    onStop={handleStopSandbox}
                    onRemove={handleRemoveSandbox}
                    onOpenFolder={handleOpenSandboxFolder}
                  />
                </div>
              )}

              {tabConditions.hasTerminal && (
                <div className={paneClass('terminal')}>
                  <LocalTerminalPanel
                    terminals={localTerminals.terminals}
                    selectedId={localTerminals.selectedId}
                    onSelect={localTerminals.setSelectedId}
                    onNew={() => void openTerminalTab()}
                    onClose={localTerminals.closeTerminal}
                    onExit={localTerminals.markExited}
                    active={activeTab === 'terminal'}
                  />
                </div>
              )}

              {activeTab === 'todo' && (
                <div className={paneClass('todo')}>
                  <TodoListView
                    todoList={todoList}
                    onRefresh={refreshTodoList}
                    loading={todoLoading}
                  />
                </div>
              )}

              {activeTab === 'cost' && (
                <div className={paneClass('cost')}>
                  <TokenAnalyticsView messages={messages} modelId={activeModelId || ''} />
                </div>
              )}

              {activeTab === 'systemPrompt' && (
                <div className={paneClass('systemPrompt')}>
                  <SystemPromptView systemPrompt={activeSystemPrompt} />
                </div>
              )}
            </div>

            {/* 入力フォーム - どのタブでも下に固定 */}
            <div className="shrink-0">
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
                memoryToggle={
                  isHelpSession
                    ? undefined
                    : {
                        checked: useMemory,
                        enabledCount: enabledMemoryCount,
                        disabled: !currentSessionId,
                        onChange: handleUseMemoryChange
                      }
                }
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
                  onOpenPanel: () => selectTab('sandbox')
                }}
                filesPanel={
                  projectPath
                    ? {
                        isOpen: activeTab === 'files',
                        onToggle: toggleFilesTab
                      }
                    : undefined
                }
                terminal={{
                  isActive: activeTab === 'terminal',
                  disabled: !currentSessionId,
                  onOpen: () => void openTerminalTab()
                }}
              />
            </div>
          </div>
        </div>
      </FilesPanelContext.Provider>
    </HelpSessionProvider>
  )
}

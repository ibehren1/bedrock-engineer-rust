import type { Message } from '@aws-sdk/client-bedrock-runtime'

import { IdentifiableMessage } from '@/types/chat/message'
import { Modal } from 'flowbite-react'
import { MetadataViewer } from '../MetadataViewer'
import { useState, useRef, useEffect, memo, useCallback } from 'react'
import { Avatar } from './Avatar'
import { Accordion } from 'flowbite-react'
import { JSONCodeBlock } from '../CodeBlocks/JSONCodeBlock'
import { TextCodeBlock } from '../CodeBlocks/TextCodeBlock'
import { TaskListCard } from '../CodeInterpreter/TaskListCard'
import CodeRenderer from '../Code/CodeRenderer'
import { toolIcons } from '@renderer/components/icons/ToolIcons'
import { FiTrash2, FiCopy } from 'react-icons/fi'
import { LuFileText } from 'react-icons/lu'
import { useTranslation } from 'react-i18next'
import useSetting from '@renderer/hooks/useSetting'
import toast from 'react-hot-toast'
import { renderToStaticMarkup } from 'react-dom/server'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { ReasoningContent } from '../CodeBlocks/Reasoning/ReasoningContent'
import { GuardContent } from '../CodeBlocks/GuardContent'

type ChatMessageProps = {
  message: IdentifiableMessage
  onDeleteMessage?: () => void
  reasoning: boolean
  /** True for the last message in the list — used to anchor streaming auto-scroll. */
  isLast?: boolean
}

// Helper function to convert various image data formats to data URL
function convertImageToDataUrl(imageData: any, format: string = 'png'): string {
  if (!imageData) return ''

  // If it's already a base64 string
  if (typeof imageData === 'string') {
    // Check if it's already a data URL
    if (imageData.startsWith('data:')) {
      return imageData
    }
    // Convert base64 to data URL
    return `data:image/${format};base64,${imageData}`
  }

  // If it's a Uint8Array
  if (imageData instanceof Uint8Array) {
    // Convert Uint8Array to base64
    const binary = Array.from(imageData)
      .map((byte) => String.fromCharCode(byte))
      .join('')
    const base64 = btoa(binary)
    return `data:image/${format};base64,${base64}`
  }

  // If it's a plain object (serialized Uint8Array)
  if (typeof imageData === 'object' && 'bytes' in imageData) {
    return convertImageToDataUrl(imageData.bytes, format)
  }

  console.warn('Unsupported image data format:', imageData)
  return ''
}

// ChatMessageをメモ化
export const ChatMessage = memo(function ChatMessage({
  message,
  onDeleteMessage,
  reasoning,
  isLast
}: ChatMessageProps) {
  const { t } = useTranslation()
  const { userName } = useSetting()
  const [isDropdownOpen, setIsDropdownOpen] = useState(false)
  const [showMetadataModal, setShowMetadataModal] = useState(false)
  const avatarRef = useRef<HTMLDivElement>(null)

  // メッセージの内容をテキスト形式で抽出する関数
  const extractMessageText = (message: Message): string => {
    if (!message.content) return ''

    // メッセージ内容をテキストとして連結
    return message.content
      .map((block) => {
        if ('text' in block) {
          return block.text || ''
        } else if ('toolUse' in block && block.toolUse?.input) {
          return `Tool: ${block.toolUse.name}\nInput: ${JSON.stringify(block.toolUse.input, null, 2)}`
        } else if ('toolResult' in block) {
          if (block.toolResult?.content) {
            return block.toolResult.content
              .map((content) => {
                if ('text' in content) return content.text
                if ('json' in content) return JSON.stringify(content.json, null, 2)
                return ''
              })
              .join('\n')
          }
          return `Tool Result: ${block.toolResult?.status}`
        }
        return ''
      })
      .join('\n\n')
  }

  const handleCopyMessage = useCallback(() => {
    const textToCopy = extractMessageText(message)
    navigator.clipboard
      .writeText(textToCopy)
      .then(() => {
        toast.success(t('Message copied to clipboard'))
        setIsDropdownOpen(false)
      })
      .catch((err) => {
        console.error('Failed to copy text: ', err)
        toast.error(t('Failed to copy message'))
      })
  }, [message, t])

  // リッチテキストとしてコピーする。Markdown を HTML に変換し、text/html と text/plain の
  // 両方をクリップボードに書き込むことで、リッチテキスト対応エディタでは書式付き（太字・斜体・
  // 箇条書きなど）で、プレーンテキストエディタでは元の Markdown として貼り付けられる。
  const handleCopyRichText = useCallback(async () => {
    const markdown = extractMessageText(message)
    try {
      const html = renderToStaticMarkup(<Markdown remarkPlugins={[remarkGfm]}>{markdown}</Markdown>)
      await navigator.clipboard.write([
        new ClipboardItem({
          'text/html': new Blob([html], { type: 'text/html' }),
          'text/plain': new Blob([markdown], { type: 'text/plain' })
        })
      ])
      toast.success(t('Message copied to clipboard'))
      setIsDropdownOpen(false)
    } catch (err) {
      console.error('Failed to copy rich text: ', err)
      toast.error(t('Failed to copy message'))
    }
  }, [message, t])

  const handleDeleteMessage = useCallback(() => {
    if (onDeleteMessage) {
      if (window.confirm(t('Are you sure you want to delete this message?'))) {
        onDeleteMessage()
        toast.success(t('Message deleted successfully'))
        setIsDropdownOpen(false)
      }
    }
  }, [onDeleteMessage, t])

  // 外部クリック時にドロップダウンを閉じる
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (avatarRef.current && !avatarRef.current.contains(event.target as Node)) {
        setIsDropdownOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [avatarRef])

  // Index of the first text block — anchors streaming auto-scroll on the last message,
  // i.e. the first line of the main response (after any reasoning / tool-use blocks).
  const firstTextBlockIndex =
    message.content?.findIndex((c) => 'text' in c && (c as { text?: string }).text !== undefined) ??
    -1

  return (
    <div className="flex gap-4 relative">
      <div className="relative" ref={avatarRef}>
        <div
          className="cursor-pointer hover:bg-raised rounded-control"
          onClick={() => setIsDropdownOpen(!isDropdownOpen)}
          title={t('Click for options')}
        >
          <Avatar role={message.role} modelId={message.metadata?.modelId} />
        </div>
        {isDropdownOpen && (
          <div className="absolute left-0 mt-1 bg-surface rounded-container shadow-lg z-50 min-w-32 py-1 border border-subtle whitespace-nowrap p-1">
            <button
              className="flex items-center gap-2 px-2.5 py-1 w-full text-left text-sm hover:bg-raised rounded-control"
              onClick={handleCopyMessage}
            >
              <FiCopy className="text-accent" />
              <span className="text-ink">{t('Copy (markdown)')}</span>
            </button>
            <button
              className="flex items-center gap-2 px-2.5 py-1 w-full text-left text-sm hover:bg-raised rounded-control"
              onClick={handleCopyRichText}
            >
              <LuFileText className="text-accent" />
              <span className="text-ink">{t('Copy (rich text)')}</span>
            </button>
            <button
              className="flex items-center gap-2 px-2.5 py-1 w-full text-left text-sm hover:bg-raised rounded-control"
              onClick={handleDeleteMessage}
            >
              <FiTrash2 className="text-danger" />
              <span className="text-ink">{t('Delete message')}</span>
            </button>
          </div>
        )}
      </div>
      <div className="flex flex-col gap-2 w-full">
        <div className="flex items-center gap-2">
          <span className="text-xs text-ink-muted relative">
            {message.role === 'user' && userName ? userName : message.role}
          </span>
          {message.metadata && (
            <button
              onClick={() => setShowMetadataModal(true)}
              className="text-xs bg-raised text-ink-muted px-2 py-0.5 rounded-control hover:bg-sunken"
            >
              metadata
            </button>
          )}
        </div>
        {message.content?.map((c, index) => {
          if ('text' in c) {
            return (
              <div
                key={index}
                className="relative"
                data-message-text="true"
                data-answer-anchor={isLast && index === firstTextBlockIndex ? 'true' : undefined}
              >
                <CodeRenderer text={c.text} />
              </div>
            )
          } else if ('guardContent' in c) {
            return (
              <div key={index} className="relative">
                <GuardContent content={c.guardContent} />
              </div>
            )
          } else if ('reasoningContent' in c) {
            const isLoading =
              reasoning && !!message.content?.length && message.content[1].text === ''
            return (
              <div key={index} className="relative">
                <ReasoningContent content={c.reasoningContent} isLoading={isLoading} />
              </div>
            )
          } else if ('toolUse' in c) {
            return (
              <div key={index} className="flex flex-col gap-2 text-xs w-full relative">
                <Accordion className="w-full" collapseAll>
                  <Accordion.Panel>
                    <Accordion.Title>
                      <div className="flex gap-3 items-center">
                        <span>{toolIcons[c.toolUse?.name || 'unknown']}</span>
                        <div className="flex gap-2">
                          <span>ToolUse:</span>
                          <span className="border rounded-control bg-raised px-2 text-ink">
                            {c.toolUse?.name}
                          </span>
                          <span>{c.toolUse?.toolUseId}</span>
                        </div>
                      </div>
                    </Accordion.Title>
                    <Accordion.Content>
                      <JSONCodeBlock json={c.toolUse?.input} />
                    </Accordion.Content>
                  </Accordion.Panel>
                </Accordion>
              </div>
            )
          } else if ('toolResult' in c) {
            return (
              <div key={index} className="flex flex-col gap-2 text-xs w-full relative">
                <Accordion className="w-full" collapseAll>
                  <Accordion.Panel>
                    <Accordion.Title>
                      {/* Status was a 24px saturated checkmark plus a solid
                          bg-green-500 / bg-red-700 chip on every single tool
                          result. Now a 5px dot and a quiet tinted badge, so a
                          long run of successful calls reads as calm and a
                          failure actually stands out. */}
                      <div className="flex gap-3 items-center">
                        <span
                          className={`size-1.5 shrink-0 rounded-full ${
                            c.toolResult?.status === 'success' ? 'bg-success' : 'bg-danger'
                          }`}
                        />
                        <span
                          className={`rounded-control px-1.5 py-0.5 text-micro ${
                            c.toolResult?.status === 'success'
                              ? 'bg-success/10 text-success'
                              : 'bg-danger/10 text-danger'
                          }`}
                        >
                          {c.toolResult?.status}
                        </span>
                        <span className="font-mono text-[10.5px] text-ink-faint">
                          {c.toolResult?.toolUseId}
                        </span>
                      </div>
                    </Accordion.Title>
                    <Accordion.Content className="w-full">
                      {c.toolResult?.content?.map((content, index) => {
                        if ('text' in content) {
                          return <TextCodeBlock key={index} text={content.text ?? ''} />
                        } else if ('json' in content) {
                          // Check if this is a TaskListResult from CodeInterpreter
                          const json = content.json
                          if (
                            json &&
                            typeof json === 'object' &&
                            'name' in json &&
                            json.name === 'codeInterpreter' &&
                            'operation' in json &&
                            json.operation === 'list'
                          ) {
                            return <TaskListCard key={index} result={json as any} />
                          }
                          return <JSONCodeBlock key={index} json={content.json} />
                        } else {
                          throw new Error('Invalid tool result content')
                        }
                      })}
                    </Accordion.Content>
                  </Accordion.Panel>
                </Accordion>
              </div>
            )
          } else if ('image' in c) {
            const imageUrl = convertImageToDataUrl(c.image?.source?.bytes)
            return (
              <div key={index} className="max-w-lg relative">
                <img
                  src={imageUrl}
                  alt="image"
                  className="rounded-container shadow-sm max-h-[512px] object-contain"
                />
              </div>
            )
          } else {
            console.error(c)
            console.error('Invalid message content')
            return (
              <div key={index} className="relative">
                <CodeRenderer text={JSON.stringify(c)} />
              </div>
            )
          }
        })}
      </div>

      <Modal
        show={showMetadataModal}
        onClose={() => setShowMetadataModal(false)}
        size="4xl"
        className="metadata-modal"
        dismissible
      >
        <Modal.Header>
          <div className="text-heading font-medium">{t('Metadata')}</div>
        </Modal.Header>
        <Modal.Body className="max-h-[80vh] overflow-auto">
          {message.metadata && <MetadataViewer metadata={message.metadata} />}
        </Modal.Body>
        <Modal.Footer>
          <button
            onClick={() => setShowMetadataModal(false)}
            className="px-5 py-2.5 text-sm font-medium bg-accent text-accent-fg rounded-container hover:bg-accent-strong focus:ring-4 focus:ring-accent transition-all"
          >
            {t('Close')}
          </button>
        </Modal.Footer>
      </Modal>
    </div>
  )
})

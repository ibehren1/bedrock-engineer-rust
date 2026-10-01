import React, { useCallback, useState, useMemo, useRef, useEffect } from 'react'
import { FiLoader, FiSend, FiX } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import toast from 'react-hot-toast'
import { AgentSelector } from '../AgentSelector'
import { ModelSelector } from '../ModelSelector'
import { ThinkingModeSelector } from '../ThinkingModeSelector'
import { InterleaveThinkingToggle } from '../InterleaveThinkingToggle'
import { PlanActToggle } from './PlanActToggle'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { AgentMentionPopup } from './AgentMentionPopup'
import { useAgentMention } from './useAgentMention'
import { useHelpSession } from '../../lib/helpSession'

export type AttachedImage = {
  file: File
  preview: string
  base64: string
}

type TextAreaProps = {
  value: string
  onChange: (value: string) => void
  onSubmit: (value: string, images: AttachedImage[]) => void
  disabled?: boolean
  isComposing: boolean
  setIsComposing: (value: boolean) => void
  sendMsgKey?: 'Enter' | 'Cmd+Enter'
  onHeightChange?: (height: number) => void
  /**
   * Chat page only: hand dropped and pasted files to the chat's attachments folder instead of
   * carrying them in memory. When absent (the website / diagram / step-functions generators)
   * the in-memory `attachedImages` path below is used unchanged.
   */
  onAddFiles?: (files: File[]) => Promise<void>
}

export const TextArea: React.FC<TextAreaProps> = ({
  value,
  onChange,
  onSubmit,
  disabled = false,
  isComposing,
  setIsComposing,
  sendMsgKey = 'Enter',
  onHeightChange,
  onAddFiles
}) => {
  const { t } = useTranslation()
  const { planMode, setPlanMode, agents } = useSettings()
  const helpSession = useHelpSession()
  const [dragActive, setDragActive] = useState(false)
  const [attachedImages, setAttachedImages] = useState<AttachedImage[]>([])
  const [isManuallyResized, setIsManuallyResized] = useState(false)
  const [textareaHeight, setTextareaHeight] = useState<number>(72) // Initial height for 3 lines (24px * 3)
  const [isHovering, setIsHovering] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const mention = useAgentMention(textareaRef, value, onChange)

  // プラットフォームに応じた Modifire キーの表示を決定
  const modifierKey = useMemo(() => {
    const isMac = navigator.platform.toLowerCase().includes('mac')
    return isMac ? '⌘' : 'Ctrl'
  }, [])

  // プレースホルダーテキストの生成
  const placeholder = useMemo(() => {
    return t('textarea.placeholder', { modifier: modifierKey })
  }, [t, modifierKey])

  // グローバルなキーボードショートカットのイベントリスナーを設定
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      // Cmd+Shift+A (または Ctrl+Shift+A) でPlan/Actモードを切り替え
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === 'a') {
        e.preventDefault()
        setPlanMode(!planMode)
      }
    }

    window.addEventListener('keydown', handleGlobalKeyDown)
    return () => {
      window.removeEventListener('keydown', handleGlobalKeyDown)
    }
  }, [planMode, setPlanMode, t])

  // テキストエリアの高さを自動調整する（10行まで）
  useEffect(() => {
    const textarea = textareaRef.current
    if (!textarea) return

    const handleMouseDown = (e: MouseEvent) => {
      // Detect mouse down on the resize handle
      const { clientX, clientY } = e
      const { bottom, right } = textarea.getBoundingClientRect()
      const resizeHandleSize = 16 // Size of the resize handle (pixels)

      // Check if the mouse is in the bottom-right corner of the textarea (resize handle)
      if (
        clientX > right - resizeHandleSize &&
        clientX < right &&
        clientY > bottom - resizeHandleSize &&
        clientY < bottom
      ) {
        const handleMouseUp = () => {
          setIsManuallyResized(true)
          document.removeEventListener('mouseup', handleMouseUp)
        }
        document.addEventListener('mouseup', handleMouseUp)
      }
    }

    textarea.addEventListener('mousedown', handleMouseDown)
    return () => {
      textarea.removeEventListener('mousedown', handleMouseDown)
    }
  }, [])

  // Automatically adjust textarea height (only if not manually resized by user)
  useEffect(() => {
    if (textareaRef.current && !isManuallyResized) {
      // Resize to the scroll height (minimum 3 lines, maximum 10 lines)
      textareaRef.current.style.height = 'auto'
      const lineHeight = 24 // Approximately 24px per line
      const minHeight = 3 * lineHeight // Height for 3 lines
      const maxHeight = 10 * lineHeight // Height for 10 lines (will scroll beyond this)
      const scrollHeight = textareaRef.current.scrollHeight

      // Limit height and change overflow settings if exceeding 10 lines
      let newHeight: number
      if (scrollHeight > maxHeight) {
        newHeight = maxHeight
        textareaRef.current.style.height = `${newHeight}px`
        textareaRef.current.style.overflowY = 'auto' // Show scrollbar
      } else {
        newHeight = Math.max(minHeight, scrollHeight)
        textareaRef.current.style.height = `${newHeight}px`
        textareaRef.current.style.overflowY = 'hidden' // Hide scrollbar
      }

      // Update height state and notify parent
      setTextareaHeight(newHeight)
      if (onHeightChange) {
        onHeightChange(newHeight)
      }
    }
  }, [value, isManuallyResized, onHeightChange])

  // Automatically scroll to the bottom when the value changes (to follow new lines)
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.scrollTop = textareaRef.current.scrollHeight
    }
  }, [value])

  // No scroll position monitoring needed as we're keeping the border visible at all times

  const validateAndProcessImage = useCallback(
    (file: File) => {
      if (file.size > 3.75 * 1024 * 1024) {
        toast.error(t('textarea.imageValidation.tooLarge'))
        return
      }

      const reader = new FileReader()
      reader.onload = () => {
        const base64 = reader.result as string
        const img = new Image()
        img.onload = () => {
          if (img.width > 8000 || img.height > 8000) {
            toast.error(t('textarea.imageValidation.dimensionTooLarge'))
            return
          }
          setAttachedImages((prev) => [
            ...prev,
            {
              file,
              preview: base64,
              base64: base64.split(',')[1]
            }
          ])
        }
        img.src = base64
      }
      reader.readAsDataURL(file)
    },
    [t]
  )

  const handlePaste = useCallback(
    async (e: React.ClipboardEvent) => {
      const items = e.clipboardData?.items
      if (!items) return

      // Check for text data (prioritize text over images)
      const hasTextData = Array.from(items).some(
        (item) => item.type === 'text/plain' || item.type === 'text/html'
      )

      // If text data exists, allow default paste behavior (paste as text)
      if (hasTextData) {
        return
      }

      // Only process as images if no text data exists
      const imageItems = Array.from(items).filter((item) => item.type.indexOf('image') !== -1)

      if (imageItems.length === 0) return

      // Prevent default paste behavior when handling as images
      e.preventDefault()

      // Chat page: the pasted image becomes a file in the chat's attachments folder, and the
      // size / format / count checks run in the main process.
      if (onAddFiles) {
        const files = imageItems
          .map((item) => item.getAsFile())
          .filter((file): file is File => file !== null)
        if (files.length > 0) await onAddFiles(files)
        return
      }

      if (attachedImages.length + imageItems.length > 20) {
        toast.error(t('textarea.imageValidation.tooManyImages'))
        return
      }

      for (const item of imageItems) {
        const file = item.getAsFile()
        if (!file) continue

        const fileType = file.type.split('/')[1].toLowerCase()
        if (!['png', 'jpeg', 'jpg', 'gif', 'webp'].includes(fileType)) {
          toast.error(t('textarea.imageValidation.unsupportedFormat', { format: fileType }))
          continue
        }

        validateAndProcessImage(file)
      }
    },
    [attachedImages.length, validateAndProcessImage, t, onAddFiles]
  )

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // メッセージ送信のキー入力処理
    if (isComposing) {
      return
    }

    // @メンションのポップアップが開いている間は送信より先にキーを処理する
    if (mention.open) {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        e.stopPropagation()
        mention.moveActive(1)
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        e.stopPropagation()
        mention.moveActive(-1)
        return
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault()
        e.stopPropagation()
        mention.selectActive()
        return
      }
      if (e.key === 'Escape') {
        e.preventDefault()
        e.stopPropagation()
        mention.close()
        return
      }
    }

    const cmdenter = e.key === 'Enter' && (e.metaKey || e.ctrlKey)
    const enter = e.key === 'Enter'

    if (
      (sendMsgKey === 'Enter' && enter && !e.shiftKey) ||
      (sendMsgKey === 'Cmd+Enter' && cmdenter)
    ) {
      e.preventDefault()
      handleSubmit()
    }
  }

  const handleSubmit = () => {
    if (value.trim() === '') {
      toast.error(t('Enter at least one character of text'))
      return
    }
    if (value.trim() || attachedImages.length > 0) {
      onSubmit(value, attachedImages)
      setAttachedImages([])
    }
  }

  // カーソル位置（なければ末尾）にテキストを挿入する
  const insertTextAtCursor = useCallback(
    (text: string) => {
      if (textareaRef.current) {
        const cursorPos = textareaRef.current.selectionStart
        const currentValue = value
        onChange(currentValue.substring(0, cursorPos) + text + currentValue.substring(cursorPos))
      } else {
        onChange(value + (value ? '\n' : '') + text)
      }
    },
    [value, onChange]
  )

  const handleDrag = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setDragActive(true)
    } else if (e.type === 'dragleave') {
      setDragActive(false)
    }
  }, [])

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      setDragActive(false)

      const allFiles = Array.from(e.dataTransfer.files)

      // Chat page: every dropped file becomes a file in the chat's attachments folder,
      // whatever its type. Validation lives in the main process.
      if (onAddFiles) {
        if (allFiles.length > 0) void onAddFiles(allFiles)
        return
      }

      // 画像ファイルと非画像ファイルを分ける
      const imageFiles = allFiles.filter((file) => {
        const fileType = file.type.split('/')[0]
        return fileType === 'image'
      })

      const nonImageFiles = allFiles.filter((file) => {
        const fileType = file.type.split('/')[0]
        return fileType !== 'image'
      })

      // 画像ファイルを処理
      const validImageFiles = imageFiles.filter((file) => {
        const type = file.type.split('/')[1]?.toLowerCase()
        if (!type || !['png', 'jpeg', 'jpg', 'gif', 'webp'].includes(type)) {
          toast.error(
            t('textarea.imageValidation.unsupportedFormat', { format: type || 'unknown' })
          )
          return false
        }
        return true
      })

      if (attachedImages.length + validImageFiles.length > 20) {
        toast.error(t('textarea.imageValidation.tooManyImages'))
        return
      }

      validImageFiles.forEach(validateAndProcessImage)

      // The generators have no chat to attach a document to, so a dropped file contributes
      // only its name, the way it did when extraction failed.
      if (nonImageFiles.length > 0) {
        insertTextAtCursor(nonImageFiles.map((file) => file.name).join('\n'))
      }
    },
    [attachedImages.length, validateAndProcessImage, t, insertTextAtCursor, onAddFiles]
  )

  const removeImage = (index: number) => {
    setAttachedImages((prev) => prev.filter((_, i) => i !== index))
  }

  return (
    <div className="relative w-full">
      {/* Only the generators still hold images in memory; the chat page lists them in its
          attachments menu instead. */}
      {!onAddFiles && attachedImages.length > 0 && (
        <div className="flex flex-wrap gap-2 mb-2">
          {attachedImages.map((image, index) => (
            <div key={index} className="relative group">
              <img
                src={image.preview}
                alt={t('textarea.aria.removeImage')}
                className="w-20 h-20 object-cover rounded-container"
              />
              <button
                onClick={() => removeImage(index)}
                className="absolute -top-2 -right-2 bg-danger text-canvas rounded-full p-1 opacity-0 group-hover:opacity-100 transition-opacity"
                aria-label={t('textarea.aria.removeImage')}
              >
                <FiX size={14} />
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Container with border that wraps both textarea and controls */}
      <div
        className={`relative border border-strong rounded-container bg-surface ${
          dragActive ? 'border-accent' : ''
        }`}
        onDragEnter={handleDrag}
      >
        <div className="relative textarea-container">
          {/* Resize bar at the top */}
          <div
            className={`resize-bar h-2 w-full cursor-ns-resize rounded-t-container transition-opacity duration-200 ${
              isHovering ? 'opacity-100 bg-raised hover:bg-sunken' : 'opacity-0'
            }`}
            onMouseEnter={() => setIsHovering(true)}
            onMouseLeave={() => setIsHovering(false)}
            onMouseDown={(e) => {
              e.preventDefault()

              // Record initial position
              const startY = e.clientY
              // Get the actual height of the textarea from the DOM element (not from state)
              const startHeight = textareaRef.current
                ? textareaRef.current.clientHeight
                : textareaHeight

              // Track mouse movement
              const handleMouseMove = (moveEvent: MouseEvent) => {
                // Calculate movement distance (moving up increases height, moving down decreases height)
                const deltaY = startY - moveEvent.clientY
                // Change directly from current height (with min and max constraints)
                const newHeight = Math.max(72, Math.min(500, startHeight + deltaY))

                if (textareaRef.current) {
                  setTextareaHeight(newHeight)
                  textareaRef.current.style.height = `${newHeight}px`
                  setIsManuallyResized(true)

                  // Notify parent of height change
                  if (onHeightChange) {
                    onHeightChange(newHeight)
                  }
                }
              }

              // Handler for when the mouse button is released
              const handleMouseUp = () => {
                document.removeEventListener('mousemove', handleMouseMove)
                document.removeEventListener('mouseup', handleMouseUp)
              }

              // Add event listeners
              document.addEventListener('mousemove', handleMouseMove)
              document.addEventListener('mouseup', handleMouseUp)
            }}
          />

          {/* Textarea without border */}
          <textarea
            ref={textareaRef}
            onCompositionStart={() => setIsComposing(true)}
            onCompositionEnd={() => setIsComposing(false)}
            className="block w-full p-2.5 pb-16 text-sm text-ink border-none bg-transparent resize-none focus:outline-none focus:ring-0"
            placeholder={placeholder}
            value={value}
            onChange={(e) => {
              onChange(e.target.value)
              // 値の反映後にキャレット位置を測るため次フレームで判定する
              requestAnimationFrame(mention.refresh)
            }}
            onKeyUp={(e) => {
              // 矢印キーやクリックでキャレットだけが動いた場合にも追従する
              if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) {
                mention.refresh()
              }
            }}
            onClick={mention.refresh}
            onBlur={mention.close}
            onKeyDown={(e) => !disabled && handleKeyDown(e)}
            onPaste={(e) => {
              mention.close()
              handlePaste(e)
            }}
            onDragEnter={handleDrag}
            onDragLeave={handleDrag}
            onDragOver={handleDrag}
            onDrop={(e) => {
              mention.close()
              handleDrop(e)
            }}
            required
            rows={3}
            style={{ height: `${textareaHeight}px` }}
          />

          {mention.open && mention.coordinates && (
            <AgentMentionPopup
              items={mention.items}
              activeIndex={mention.activeIndex}
              coordinates={mention.coordinates}
              textareaRef={textareaRef}
              onSelect={mention.select}
            />
          )}
        </div>

        {/* Controls at the bottom */}
        <div className="absolute bottom-0 left-0 right-0 flex items-center justify-between px-2.5 py-1 bg-surface rounded-b-container">
          {/* The Help chat pins its own agent and model, so it shows them as plain text rather
              than pickers that would appear to change a chat they cannot change. */}
          {helpSession.isHelpSession ? (
            <div className="flex items-center gap-2.5 z-10 pointer-events-auto">
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-ink-muted">{t('inputControls.agent')}</span>
                <span className="text-xs font-medium text-ink">{helpSession.agentName}</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-ink-muted">{t('inputControls.model')}</span>
                <span className="text-xs font-medium text-ink">{helpSession.modelId}</span>
              </div>
            </div>
          ) : (
            <div className="flex items-center gap-2.5 z-10 pointer-events-auto">
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-ink-muted">{t('inputControls.agent')}</span>
                <AgentSelector agents={agents} alignment="left" />
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-ink-muted">{t('inputControls.model')}</span>
                <ModelSelector openable={true} />
              </div>
              <ThinkingModeSelector label={t('inputControls.thinking')} />
              <InterleaveThinkingToggle />
            </div>
          )}

          <div className="flex items-center gap-2">
            <div>
              <PlanActToggle />
            </div>
            <button
              onClick={handleSubmit}
              disabled={disabled}
              className={`rounded-container ${
                disabled ? '' : 'hover:bg-raised'
              } px-2 py-2 text-ink hover:bg-raised`}
              aria-label={disabled ? t('textarea.aria.sending') : t('textarea.aria.sendMessage')}
            >
              {disabled ? (
                <FiLoader className="text-base animate-spin" />
              ) : (
                <FiSend className="text-base" />
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

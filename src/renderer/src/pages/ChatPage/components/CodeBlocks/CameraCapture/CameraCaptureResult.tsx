import React, { useState, useEffect } from 'react'
import { createPortal } from 'react-dom'
import { IoIosClose } from 'react-icons/io'
import { HiOutlineCamera } from 'react-icons/hi'
import LocalImage from '@renderer/components/LocalImage'
import { useTranslation } from 'react-i18next'

export interface CameraCaptureResponse {
  success: boolean
  name: string
  message: string
  result: {
    filePath: string
    metadata: {
      width: number
      height: number
      format: string
      fileSize: number
      timestamp: string
      deviceId: string
      deviceName: string
    }
    recognition?: {
      content: string
      modelId: string
      prompt?: string
    }
  }
}

export const CameraCaptureResult: React.FC<{ response: CameraCaptureResponse }> = ({
  response
}) => {
  const { t } = useTranslation()
  const { result } = response
  const [isPromptExpanded, setIsPromptExpanded] = useState(false)
  const [isFullscreen, setIsFullscreen] = useState(false)

  // Null safety check
  if (!result) {
    return (
      <div className="w-full bg-danger text-danger p-2.5 rounded-container">
        <p>{t('Error: Camera capture result is missing')}</p>
      </div>
    )
  }

  // Default metadata values
  const metadata = result.metadata || {
    width: 0,
    height: 0,
    format: 'unknown',
    fileSize: 0,
    timestamp: new Date().toISOString(),
    deviceId: 'unknown',
    deviceName: 'Unknown Device'
  }

  // Safe file path access
  const filePath = result.filePath || ''

  // Handle Esc key to close fullscreen
  useEffect(() => {
    const handleEsc = (event: KeyboardEvent) => {
      if (event.keyCode === 27) {
        setIsFullscreen(false)
      }
    }
    window.addEventListener('keydown', handleEsc)
    return () => {
      window.removeEventListener('keydown', handleEsc)
    }
  }, [])

  // Format file size in appropriate units
  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  // Format timestamp
  const formatTimestamp = (timestamp: string): string => {
    try {
      return new Date(timestamp).toLocaleString()
    } catch {
      return timestamp
    }
  }

  return (
    <div className="w-full">
      <div className="bg-surface-2 text-ink rounded-container overflow-hidden shadow-raised border border-subtle">
        {/* Header section with title and metadata */}
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 p-2.5 pb-2 border-b border-faint">
          <h3 className="text-heading font-medium flex items-center gap-2">
            <HiOutlineCamera className="w-4 h-4 text-accent" />
            {t('Camera Capture')}
          </h3>
          <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-ink-muted">
            <span className="text-accent" title={`Device ID: ${metadata.deviceId}`}>
              {metadata.deviceName}
            </span>
            <span>
              {metadata.width} × {metadata.height}
            </span>
            <span>{formatFileSize(metadata.fileSize)}</span>
            <span>{metadata.format.toUpperCase()}</span>
            <span>{formatTimestamp(metadata.timestamp)}</span>
          </div>
        </div>

        {/* Main content area */}
        <div className="flex flex-col md:flex-row gap-4 p-2.5 pt-2">
          {/* Left side: Image display */}
          <div className="flex-shrink-0 md:w-2/5">
            <div
              onClick={() => setIsFullscreen(true)}
              className="cursor-pointer hover:opacity-80 transition-opacity"
            >
              <LocalImage
                src={filePath}
                alt="Camera capture"
                className="aspect-auto max-h-[40vh] object-contain w-full rounded-control"
              />
            </div>
            <div className="mt-2 text-xs text-ink-faint truncate" title={filePath}>
              {filePath}
            </div>

            {/* Camera device info */}
            <div className="mt-2 text-xs text-ink-faint">
              <span className="text-accent">{t('Camera Device')}:</span> {metadata.deviceName}
              {metadata.deviceId !== 'default' && (
                <span className="opacity-70 ml-1">({metadata.deviceId})</span>
              )}
            </div>

            {/* Analysis prompt under image */}
            {result.recognition?.prompt && (
              <div className="mt-3">
                <button
                  onClick={() => setIsPromptExpanded(!isPromptExpanded)}
                  className="text-xs text-ink-faint hover:text-ink-faint flex items-center gap-1 mb-2 transition-colors"
                >
                  <span
                    className={`transform transition-transform ${isPromptExpanded ? 'rotate-90' : ''}`}
                  >
                    ▶
                  </span>
                  {t('Analysis Prompt')}
                </button>
                {isPromptExpanded && (
                  <div className="bg-accent/30 text-accent p-2 rounded-control text-sm">
                    {result.recognition.prompt}
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Right side: Analysis results */}
          {result.recognition && (
            <div className="flex-1 flex flex-col h-[40vh]">
              <div className="bg-sunken p-2 rounded-control flex-1 overflow-y-auto">
                <p className="text-ink-faint whitespace-pre-wrap text-sm">
                  {result.recognition.content}
                </p>
              </div>

              <div className="text-xs text-ink-faint mt-3">
                {t('Analyzed with')}:{' '}
                <span className="font-mono">{result.recognition.modelId}</span>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Fullscreen modal */}
      {isFullscreen &&
        createPortal(
          <div
            className="fixed inset-0 bg-black bg-opacity-75"
            style={{ zIndex: 2147483647 }}
            onClick={() => setIsFullscreen(false)}
          >
            {/* Close button */}
            <div
              className="absolute top-4 right-4 p-2"
              style={{ zIndex: 2147483647 }}
              onClick={() => setIsFullscreen(false)}
            >
              <IoIosClose className="text-white w-8 h-8 hover:bg-white/20 rounded-control cursor-pointer" />
            </div>

            {/* Image content */}
            <div
              className="w-full h-full flex items-center justify-center p-8"
              onClick={(e) => e.stopPropagation()}
            >
              <LocalImage
                src={filePath}
                alt="Camera capture"
                className="max-w-full max-h-full object-contain"
              />
            </div>
          </div>,
          document.body
        )}
    </div>
  )
}

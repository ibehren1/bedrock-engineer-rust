import React, { useState, useEffect } from 'react'
import { FiAlertCircle, FiImage } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import { FiMaximize2, FiMinimize2, FiDownload } from 'react-icons/fi'

interface ImagePreviewProps {
  filename: string
  imagePath: string
}

export const ImagePreview: React.FC<ImagePreviewProps> = ({ filename, imagePath }) => {
  const { t } = useTranslation('tools')
  const [isFullscreen, setIsFullscreen] = useState(false)
  const [imageError, setImageError] = useState(false)

  const handleDownload = () => {
    try {
      // Create download link
      const link = document.createElement('a')
      link.href = imagePath // This is now a Base64 data URL
      link.download = filename
      document.body.appendChild(link)
      link.click()
      document.body.removeChild(link)
    } catch (error) {
      console.error('Failed to download image:', error)
    }
  }

  const handleImageError = () => {
    setImageError(true)
  }

  const toggleFullscreen = () => {
    setIsFullscreen(!isFullscreen)
  }

  // Escキーで全画面表示を終了
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && isFullscreen) {
        setIsFullscreen(false)
      }
    }

    if (isFullscreen) {
      document.addEventListener('keydown', handleKeyDown)
      return () => {
        document.removeEventListener('keydown', handleKeyDown)
      }
    }

    // isFullscreenがfalseの場合はクリーンアップ関数を返さない
    return undefined
  }, [isFullscreen])

  if (imageError) {
    return (
      <div className="flex flex-col items-center justify-center p-8 bg-surface-2 border border-subtle rounded-container">
        <FiAlertCircle className="w-4 h-4 mb-3 text-danger" />
        <p className="text-ink-muted text-sm text-center">
          {t('code interpreter display.Failed to load image', 'Failed to load image')}
        </p>
        <p className="text-ink-faint text-xs mt-1">{filename}</p>
      </div>
    )
  }

  return (
    <div className="space-y-3">
      {/* Image Header */}
      <div className="flex items-center justify-between p-3 bg-surface-2 border border-subtle rounded-t-container">
        <div className="flex items-center gap-2">
          <FiImage className="w-4 h-4 text-ink-muted" />
          <span className="text-sm font-medium text-ink">{filename}</span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={toggleFullscreen}
            className="p-1 text-ink-muted hover:text-ink hover:bg-raised rounded-control transition-colors"
            title={
              isFullscreen
                ? t('code interpreter display.Minimize', 'Minimize')
                : t('code interpreter display.Maximize', 'Maximize')
            }
          >
            {isFullscreen ? <FiMinimize2 className="size-4" /> : <FiMaximize2 className="size-4" />}
          </button>
          <button
            onClick={handleDownload}
            className="p-1 text-ink-muted hover:text-ink hover:bg-raised rounded-control transition-colors"
            title={t('code interpreter display.Download image', 'Download image')}
          >
            <FiDownload className="size-4" />
          </button>
        </div>
      </div>

      {/* Image Display */}
      <div
        className={`border border-subtle rounded-b-container overflow-hidden bg-surface ${
          isFullscreen ? 'fixed inset-4 z-50 shadow-2xl' : ''
        }`}
      >
        <div
          className={`flex justify-center ${isFullscreen ? 'h-full' : 'max-h-96'} overflow-auto`}
        >
          <img
            src={imagePath}
            alt={filename}
            onError={handleImageError}
            onClick={toggleFullscreen}
            className={`object-contain cursor-pointer ${
              isFullscreen ? 'max-w-full max-h-full' : 'max-w-full h-auto'
            }`}
            style={{
              maxHeight: isFullscreen ? '100%' : '384px' // 96 * 4px = 384px
            }}
            title={
              isFullscreen
                ? t('code interpreter display.Click to minimize', 'Click to minimize')
                : t('code interpreter display.Click to maximize', 'Click to maximize')
            }
          />
        </div>
      </div>

      {/* Fullscreen Backdrop */}
      {isFullscreen && (
        <div className="fixed inset-0 bg-black bg-opacity-50 z-40" onClick={toggleFullscreen} />
      )}
    </div>
  )
}

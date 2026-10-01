import React, { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { ImagePreview } from './ImagePreview'

interface FileDisplayProps {
  files: string[]
}

export const FileDisplay: React.FC<FileDisplayProps> = ({ files }) => {
  const { t } = useTranslation('tools')
  const [previewingImages, setPreviewingImages] = useState<Record<string, boolean>>({})
  const [imageDataUrls, setImageDataUrls] = useState<Record<string, string>>({})
  const [_loadingImages, setLoadingImages] = useState<Record<string, boolean>>({})

  // Extract filename from absolute path
  const getFileName = (filePath: string): string => {
    return filePath.split('/').pop() || filePath.split('\\').pop() || filePath
  }

  // 画像ファイルかどうかの判定（絶対パス対応）
  const isImageFile = (filePath: string) => {
    const filename = getFileName(filePath)
    const ext = filename.toLowerCase().split('.').pop() || ''
    return ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'].includes(ext)
  }

  // Base64画像データを取得（絶対パス対応）
  const getImageDataUrl = async (filePath: string): Promise<string> => {
    try {
      // filePath is now absolute path, use it directly
      const base64DataUrl = await window.api.images.getLocalImage(filePath)
      return base64DataUrl
    } catch (error) {
      console.error('Failed to load image as base64:', error)
      throw error
    }
  }

  // 画像ファイルを自動的に読み込み、常に表示する
  useEffect(() => {
    const loadImageFiles = async () => {
      const imageFiles = files.filter(isImageFile)

      // 全ての画像ファイルを自動的にプレビュー状態にする
      const newPreviewingImages: Record<string, boolean> = {}
      imageFiles.forEach((filePath) => {
        newPreviewingImages[filePath] = true
      })
      setPreviewingImages(newPreviewingImages)

      // 画像データを並行して読み込み
      const loadPromises = imageFiles.map(async (filePath) => {
        if (!imageDataUrls[filePath]) {
          setLoadingImages((prev) => ({ ...prev, [filePath]: true }))
          try {
            const dataUrl = await getImageDataUrl(filePath)
            setImageDataUrls((prev) => ({
              ...prev,
              [filePath]: dataUrl
            }))
          } catch (error) {
            console.error(`Failed to load image ${filePath}:`, error)
            // エラーが発生した場合はプレビューを無効にする
            setPreviewingImages((prev) => ({
              ...prev,
              [filePath]: false
            }))
          } finally {
            setLoadingImages((prev) => ({ ...prev, [filePath]: false }))
          }
        }
      })

      await Promise.all(loadPromises)
    }

    if (files.length > 0) {
      loadImageFiles()
    }
  }, [files, imageDataUrls])

  if (!files || files.length === 0) {
    return (
      <div className="bg-surface-2 border border-subtle rounded-container p-2.5">
        <p className="text-ink-muted text-sm italic">
          {t('code interpreter display.No files generated', 'No files generated')}
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-3">
      {/* Image Previews - 画像ファイルのプレビューのみ表示 */}
      {files.map((filePath, index) => {
        const filename = getFileName(filePath)
        if (isImageFile(filePath) && previewingImages[filePath] && imageDataUrls[filePath]) {
          return (
            <div key={`preview-${index}`} className="mt-4">
              <ImagePreview filename={filename} imagePath={imageDataUrls[filePath]} />
            </div>
          )
        }
        return null
      })}

      {/* Summary */}
      <div className="text-xs text-ink-muted text-center pt-2 border-t border-subtle">
        {t('code interpreter display.Total files generated', 'Total files generated')}:{' '}
        {files.length}
      </div>
    </div>
  )
}

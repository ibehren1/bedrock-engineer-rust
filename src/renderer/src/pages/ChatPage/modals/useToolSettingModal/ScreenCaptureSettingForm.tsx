import React, { useMemo, useState, useEffect, useCallback } from 'react'
import { FiMonitor, FiSquare } from 'react-icons/fi'
import { useTranslation } from 'react-i18next'
import { Label, Select, Button } from 'flowbite-react'
import { ArrowPathIcon } from '@heroicons/react/24/outline'
import { useSettings } from '@renderer/contexts/SettingsContext'
import { WindowConfig } from '@/types/agent-chat'

// ウィンドウ情報の型定義
interface WindowInfo {
  id: string
  name: string
  enabled: boolean
  thumbnail: string // base64画像データ
  dimensions: { width: number; height: number } // 実際のウィンドウサイズ
}

export const ScreenCaptureSettingForm: React.FC = () => {
  const { t } = useTranslation()
  const {
    recognizeImageModel,
    setRecognizeImageModel,
    availableModels,
    selectedAgentId,
    getAgentAllowedWindows,
    updateAgentAllowedWindows
  } = useSettings()

  // ウィンドウ関連の状態
  const [availableWindows, setAvailableWindows] = useState<WindowInfo[]>([])
  const [allowedWindows, setAllowedWindows] = useState<WindowConfig[]>([])
  const [isLoadingWindows, setIsLoadingWindows] = useState(false)
  const [imageLoadErrors, setImageLoadErrors] = useState<Set<string>>(new Set())

  // Vision-capable モデルをフィルタリング（Claude と Nova シリーズ）
  const visionCapableModels = useMemo(() => {
    return availableModels
      .filter(
        (model) =>
          model.modelId.includes('anthropic.claude') || model.modelId.includes('amazon.nova')
      )
      .sort((a, b) => a.modelName.localeCompare(b.modelName))
  }, [availableModels])

  // エージェントの許可ウィンドウ設定を読み込み
  useEffect(() => {
    if (selectedAgentId) {
      const windows = getAgentAllowedWindows(selectedAgentId)
      setAllowedWindows(windows)
    }
  }, [selectedAgentId, getAgentAllowedWindows])

  // 画像読み込みエラーハンドラー（React状態ベース）
  const handleImageError = useCallback((windowId: string) => {
    setImageLoadErrors((prev) => new Set([...prev, windowId]))
  }, [])

  // 利用可能なウィンドウ一覧を取得
  const fetchAvailableWindows = async () => {
    setIsLoadingWindows(true)
    setImageLoadErrors(new Set()) // エラー状態をリセット
    try {
      const windows = await window.api.screen.listAvailableWindows()
      setAvailableWindows(windows || [])
    } catch (error) {
      console.error('Failed to fetch available windows:', error)
      setAvailableWindows([])
    } finally {
      setIsLoadingWindows(false)
    }
  }

  // ウィンドウが許可されているかチェック
  const isWindowAllowed = (window: WindowInfo): boolean => {
    return allowedWindows.some((allowed) => allowed.id === window.id)
  }

  // ウィンドウの許可/非許可を切り替え
  const handleWindowToggle = (window: WindowInfo, enabled: boolean) => {
    if (!selectedAgentId) return

    let updatedWindows: WindowConfig[]

    if (enabled) {
      // ウィンドウを許可リストに追加
      const newWindow: WindowConfig = {
        id: window.id,
        name: window.name,
        enabled: true
      }
      updatedWindows = [...allowedWindows.filter((w) => w.id !== window.id), newWindow]
    } else {
      // ウィンドウを許可リストから削除
      updatedWindows = allowedWindows.filter((w) => w.id !== window.id)
    }

    setAllowedWindows(updatedWindows)
    updateAgentAllowedWindows(selectedAgentId, updatedWindows)
  }

  // 初回読み込み時にウィンドウ一覧を取得
  useEffect(() => {
    fetchAvailableWindows()
  }, [])

  return (
    <div className="max-w-none w-full">
      {/* ツールの説明 */}
      <div className="mb-3 w-full">
        <p className="mb-4 text-ink">
          {t(
            'tool info.screenCapture.description',
            'The screenCapture tool captures the current screen and saves it as an image file. When a recognition prompt is provided, the captured image will be automatically analyzed with AI to extract text content, identify UI elements, and provide detailed visual descriptions for debugging and documentation purposes.'
          )}
        </p>
      </div>

      {/* 設定フォーム */}
      <div className="flex flex-col gap-4 p-2.5 border border-subtle rounded-control mb-3 w-full">
        <h4 className="font-medium text-sm mb-2 text-ink">{t('AI Image Analysis Settings')}</h4>

        {/* LLMモデル選択 */}
        <div className="w-full">
          <Label htmlFor="recognizeImageModel" value={t('AI Model for Image Analysis')} />
          <Select
            id="recognizeImageModel"
            value={recognizeImageModel}
            onChange={(e) => setRecognizeImageModel(e.target.value)}
            className="mt-2 w-full"
          >
            {visionCapableModels.map((model) => (
              <option key={model.modelId} value={model.modelId}>
                {model.modelName}
              </option>
            ))}
          </Select>
        </div>

        {/* 使用方法 */}
        <div className="mt-4 p-3 bg-accent-tint border border-accent rounded-control">
          <h5 className="font-medium mb-2 text-accent">{t('How to Use')}</h5>
          <ul className="text-sm text-ink space-y-1">
            <li>
              • <strong>{t('Screen capture only')}:</strong>{' '}
              {t('Use without any prompt to capture screen image only')}
            </li>
            <li>
              • <strong>{t('Screen capture + AI analysis')}:</strong>{' '}
              {t('Provide a recognition prompt to automatically analyze the captured image')}
            </li>
            <li>
              • <strong>{t('Example prompts')}:</strong>{' '}
              {t(
                '"Describe this error screen", "Extract all text from this image", "Analyze the UI layout"'
              )}
            </li>
          </ul>
        </div>

        {/* プラットフォーム要件 */}
        <div className="mt-4 p-3 bg-success-soft border border-success rounded-control">
          <h5 className="font-medium mb-2 text-success">{t('Platform Requirements')}</h5>
          <ul className="text-sm text-ink space-y-1">
            <li>
              • <strong>macOS:</strong>{' '}
              {t(
                'Screen Recording permission required in System Preferences > Security & Privacy > Privacy > Screen Recording'
              )}
            </li>
          </ul>
        </div>
      </div>

      {/* ウィンドウアクセス許可設定 */}
      <div className="flex flex-col gap-4 p-2.5 border border-subtle rounded-control mb-3 w-full">
        <div className="flex items-center justify-between">
          <h4 className="font-medium text-sm mb-2 text-ink">{t('Window Access Permissions')}</h4>
          <Button
            size="sm"
            color="light"
            onClick={fetchAvailableWindows}
            disabled={isLoadingWindows}
            className="flex items-center gap-2"
          >
            <ArrowPathIcon className={`w-4 h-4 ${isLoadingWindows ? 'animate-spin' : ''}`} />
            {t('Refresh')}
          </Button>
        </div>

        <p className="text-sm text-ink-muted">
          {t(
            'Select which application windows this agent is allowed to capture. Only selected windows can be targeted for screenshot capture.'
          )}
        </p>

        {/* ウィンドウプレビューグリッド */}
        {isLoadingWindows ? (
          <div className="flex items-center justify-center py-8">
            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-accent"></div>
            <span className="ml-3 text-ink-muted">{t('Loading window previews...')}</span>
          </div>
        ) : availableWindows.length === 0 ? (
          <div className="text-center py-8 text-ink-muted">
            <FiMonitor className="w-4 h-4 mb-2 text-ink-faint" />
            <p>{t('No windows available. Click refresh to try again.')}</p>
          </div>
        ) : (
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 max-h-96 overflow-y-auto p-2">
            {availableWindows.map((window) => {
              const isSelected = isWindowAllowed(window)
              const hasImageError = imageLoadErrors.has(window.id)

              return (
                <div
                  key={window.id}
                  className={`
                    relative cursor-pointer transition-all duration-200 transform hover:scale-105
                    ${
                      isSelected
                        ? 'ring-2 ring-accent shadow-lg'
                        : 'ring-1 ring-subtle hover:ring-subtle'
                    }
                    rounded-container overflow-hidden bg-surface
                  `}
                  onClick={() => handleWindowToggle(window, !isSelected)}
                >
                  {/* サムネイル画像 */}
                  <div className="relative aspect-video bg-raised">
                    {hasImageError ? (
                      // React状態ベースのフォールバックUI
                      <div className="flex items-center justify-center h-full text-ink-faint">
                        <div className="text-center">
                          <FiSquare className="w-4 h-4 mb-1 text-ink-faint" />
                          <div className="text-xs">Preview not available</div>
                        </div>
                      </div>
                    ) : (
                      <img
                        src={window.thumbnail}
                        alt={`Preview of ${window.name}`}
                        className="w-full h-full object-cover"
                        onError={() => handleImageError(window.id)}
                      />
                    )}

                    {/* 選択状態のオーバーレイ */}
                    {isSelected && (
                      <div className="absolute inset-0 bg-accent-tint/20 flex items-center justify-center">
                        <div className="bg-accent text-accent-fg rounded-full p-1">
                          <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
                            <path
                              fillRule="evenodd"
                              d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                              clipRule="evenodd"
                            />
                          </svg>
                        </div>
                      </div>
                    )}

                    {/* ウィンドウサイズ情報 */}
                    <div className="absolute top-1 right-1 bg-black/50 text-white text-xs px-1 py-0.5 rounded-control">
                      {window.dimensions.width}×{window.dimensions.height}
                    </div>
                  </div>

                  {/* ウィンドウ情報 */}
                  <div className="p-3">
                    <div className="font-medium text-sm text-ink truncate">{window.name}</div>
                  </div>
                </div>
              )
            })}
          </div>
        )}

        {/* 許可されたウィンドウの数 */}
        {allowedWindows.length > 0 && (
          <div className="text-sm text-success">
            {t('{{count}} window(s) allowed', { count: allowedWindows.length })}
          </div>
        )}

        {/* ヒント */}
        <div className="mt-4 p-3 bg-warning-soft border border-warning rounded-control">
          <h5 className="font-medium mb-2 text-warning">{t('Usage Tips')}</h5>
          <ul className="text-sm text-ink space-y-1">
            <li>• {t('If no windows are selected, the agent can capture the full screen')}</li>
            <li>• {t('Window permissions are checked each time before capture')}</li>
            <li>• {t('Use the refresh button to update the list of available windows')}</li>
          </ul>
        </div>
      </div>
    </div>
  )
}

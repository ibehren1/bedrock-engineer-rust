import React from 'react'
import { useTranslation } from 'react-i18next'
import { JSONCodeBlock } from '../CodeBlocks/JSONCodeBlock'
import { CheckCircleIcon, ServerIcon, CurrencyDollarIcon } from '@heroicons/react/24/outline'
import { PricingCalculator } from '@common/models/pricing'
import { IdentifiableMessage } from '@/types/chat/message'

interface MetadataViewerProps {
  metadata: IdentifiableMessage['metadata']
}

export const MetadataViewer: React.FC<MetadataViewerProps> = ({ metadata }) => {
  const { t, i18n } = useTranslation()
  const currencyLocale = i18n.language === 'ja' ? 'ja-JP' : 'en-US'

  if (!metadata) {
    return <div className="text-ink-muted">{t('No metadata available')}</div>
  }

  return (
    <div className="text-sm flex flex-col gap-3">
      {metadata?.converseMetadata?.usage && (
        <div className="flex flex-col gap-3">
          <h3 className="font-medium text-base border-b pb-2 text-ink">{t('Token Usage')}</h3>
          <div className="grid grid-cols-3 gap-4 bg-surface-2 p-2.5 rounded-container">
            <div className="flex flex-col">
              <span className="text-ink-muted">{t('Input')}</span>
              <span className="font-medium text-heading text-ink">
                {metadata.converseMetadata.usage.inputTokens}
              </span>
            </div>
            <div className="flex flex-col">
              <span className="text-ink-muted">{t('Output')}</span>
              <span className="font-medium text-heading text-ink">
                {metadata.converseMetadata.usage.outputTokens}
              </span>
            </div>
            <div className="flex flex-col">
              <span className="text-ink-muted">{t('Total')}</span>
              <span className="font-medium text-heading text-ink">
                {metadata.converseMetadata.usage.totalTokens}
              </span>
            </div>
          </div>
        </div>
      )}

      {/* コスト情報セクション - 使用量がある場合のみ表示 */}
      {metadata.sessionCost && (
        <div className="flex flex-col gap-3">
          <h3 className="font-medium text-base border-b pb-2 text-ink">{t('Cost')}</h3>
          <div className="bg-surface-2 p-2.5 rounded-container">
            <div className="flex items-center">
              <CurrencyDollarIcon className="w-4 h-4 text-success mr-2" />
              <div className="flex flex-col">
                <span className="text-ink-muted">{t('Session Cost')}</span>
                <span className="font-medium text-heading text-ink">
                  {PricingCalculator.formatCurrency(metadata.sessionCost, 'USD', currencyLocale)}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* キャッシュ情報セクション - キャッシュ情報がある場合のみ表示 */}
      {metadata?.converseMetadata?.usage &&
        (metadata.converseMetadata.usage.cacheReadInputTokens !== undefined ||
          metadata.converseMetadata.usage.cacheWriteInputTokens !== undefined) && (
          <div className="flex flex-col gap-3">
            <h3 className="font-medium text-base border-b pb-2 text-ink">{t('Cache Usage')}</h3>
            <div className="grid grid-cols-2 gap-4 bg-surface-2 p-2.5 rounded-container">
              <div className="flex flex-col">
                <span className="text-ink-muted">{t('Cache Read')}</span>
                <span className="font-medium text-heading text-ink">
                  {metadata.converseMetadata.usage.cacheReadInputTokens || 0}
                </span>
              </div>
              <div className="flex flex-col">
                <span className="text-ink-muted">{t('Cache Write')}</span>
                <span className="font-medium text-heading text-ink">
                  {metadata.converseMetadata.usage.cacheWriteInputTokens || 0}
                </span>
              </div>
            </div>

            {/* キャッシュ状態インジケーター */}
            <div className="mt-1">
              {metadata.converseMetadata.usage.cacheReadInputTokens > 0 && (
                <div className="flex items-center text-success text-sm">
                  <CheckCircleIcon className="w-4 h-4 mr-1" />
                  <span>{t('Cache hit detected!')}</span>
                </div>
              )}
              {metadata.converseMetadata.usage.cacheWriteInputTokens > 0 &&
                metadata.converseMetadata.usage.cacheReadInputTokens === 0 && (
                  <div className="flex items-center text-accent text-sm">
                    <ServerIcon className="w-4 h-4 mr-1" />
                    <span>{t('New cache created')}</span>
                  </div>
                )}
            </div>
          </div>
        )}

      {metadata?.converseMetadata?.metrics && (
        <div className="flex flex-col gap-3">
          <h3 className="font-medium text-base border-b pb-2 text-ink">{t('Performance')}</h3>
          <div className="bg-surface-2 p-2.5 rounded-container">
            <div className="flex flex-col">
              <span className="text-ink-muted">{t('Latency')}</span>
              <span className="font-medium text-heading text-ink">
                {metadata.converseMetadata.metrics.latencyMs} ms
              </span>
            </div>
          </div>
        </div>
      )}

      {metadata?.converseMetadata?.trace && (
        <div className="flex flex-col gap-3">
          <h3 className="font-medium text-base border-b pb-2 text-ink">{t('Trace')}</h3>
          <div className="bg-surface-2 p-2.5 rounded-container overflow-auto max-h-64">
            <JSONCodeBlock json={metadata.converseMetadata.trace} />
          </div>
        </div>
      )}

      {metadata?.converseMetadata?.performanceConfig && (
        <div className="flex flex-col gap-3">
          <h3 className="font-medium text-base border-b pb-2 text-ink">
            {t('Performance Config')}
          </h3>
          <div className="bg-surface-2 p-2.5 rounded-container overflow-auto max-h-64">
            <JSONCodeBlock json={metadata.converseMetadata.performanceConfig} />
          </div>
        </div>
      )}
    </div>
  )
}

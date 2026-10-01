import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FaEye, FaEyeSlash } from 'react-icons/fa'
import { Button, Input } from '@renderer/components/ui'

interface ApiKeySectionProps {
  apiKey: string
  onSave: (apiKey: string) => void
}

export const ApiKeySection = ({ apiKey: initialApiKey, onSave }: ApiKeySectionProps) => {
  const { t } = useTranslation()
  const [apiKey, setApiKey] = useState(initialApiKey)
  const [showApiKey, setShowApiKey] = useState(false)

  const handleSave = () => {
    onSave(apiKey)
  }

  return (
    <div className="flex flex-col gap-2 p-2.5 border border-subtle rounded-control">
      <h4 className="font-medium text-sm mb-2 text-ink">{t('Tavily Search API Settings')}</h4>
      <div className="flex-grow">
        <label className="block text-xs text-ink-muted mb-1">API Key</label>
        <div className="flex items-center gap-2">
          <div className="flex-grow relative">
            <Input
              type={showApiKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="tvly-xxxxxxxxxxxxxxx"
              className="pr-10"
            />
            <button
              type="button"
              className="absolute inset-y-0 right-0 flex items-center px-3 text-ink-faint hover:text-ink-muted cursor-pointer"
              onClick={() => setShowApiKey(!showApiKey)}
              aria-label={showApiKey ? t('Hide API Key') : t('Show API Key')}
              title={showApiKey ? t('Hide API Key') : t('Show API Key')}
            >
              {showApiKey ? <FaEyeSlash className="w-4 h-4" /> : <FaEye className="w-4 h-4" />}
            </button>
          </div>
          <Button onClick={handleSave} variant="primary" className="cursor-pointer">
            {t('Save')}
          </Button>
        </div>
        <p className="text-xs text-ink-muted mt-2">
          {t('You need a Tavily Search API key to use this feature. Get your API key at')}
          <a
            href="https://tavily.com/"
            target="_blank"
            rel="noreferrer"
            className="ml-1 text-accent hover:underline"
          >
            tavily.com
          </a>
        </p>
      </div>
    </div>
  )
}

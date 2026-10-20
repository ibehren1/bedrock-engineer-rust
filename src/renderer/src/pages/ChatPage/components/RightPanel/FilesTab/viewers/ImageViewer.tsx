import React, { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

export const ImageViewer: React.FC<{ url: string; name: string }> = ({ url, name }) => {
  const { t } = useTranslation()
  const [failed, setFailed] = useState(false)
  const [size, setSize] = useState<{ w: number; h: number }>()
  useEffect(() => {
    setFailed(false)
    setSize(undefined)
  }, [url])

  if (failed) {
    return (
      <div className="h-full grid place-items-center text-sm text-ink-faint">
        {t('filesPanel.imageFailed')}
      </div>
    )
  }
  return (
    <div className="h-full flex flex-col items-center justify-center gap-2 p-4">
      <img
        src={url}
        alt={name}
        className="max-w-full max-h-full min-h-0 object-contain rounded-control bg-[repeating-conic-gradient(var(--surface-2)_0%_25%,transparent_0%_50%)] bg-[length:16px_16px]"
        onLoad={(e) =>
          setSize({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })
        }
        onError={() => setFailed(true)}
      />
      {size && (
        <div className="text-micro text-ink-faint shrink-0">
          {size.w} × {size.h}
        </div>
      )}
    </div>
  )
}

export default ImageViewer

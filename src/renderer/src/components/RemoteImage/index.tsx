import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useWebContentStatus } from '@renderer/lib/webContent'
import { WebContentBlocked } from '../WebContentBlocked'

/** `autoLoadRemoteImages`: load remote images in model-written content without a click. */
export const REMOTE_IMAGES_SETTING = 'autoLoadRemoteImages'

const hostOf = (src: string): string => {
  try {
    return new URL(src).host || src
  } catch {
    return src
  }
}

/**
 * An `http(s)` image from model-written content (UI-1). Loading it sends a request to whatever
 * server the URL names, and a reply steered by a web page or tool result can put chat contents in
 * that URL, so by default it shows a placeholder naming the host until the user clicks Load.
 * With the setting on (Settings → Chat), it loads straight away, as before.
 */
export const RemoteImage: React.FC<React.ImgHTMLAttributes<HTMLImageElement> & { src: string }> = ({
  src,
  alt,
  ...props
}) => {
  const { t } = useTranslation()
  const [load, setLoad] = useState<boolean>(() => !!window.store.get(REMOTE_IMAGES_SETTING))
  const web = useWebContentStatus()

  if (!web.allowed) return <WebContentBlocked reason={web.reason} compact />
  if (load) return <img src={src} alt={alt} {...props} />
  return (
    <span className="my-1 inline-flex max-w-full items-center gap-2 rounded-control border border-subtle bg-surface px-2 py-1 text-xs text-ink-muted">
      <span className="truncate">
        {t('remoteImages.placeholder', { host: hostOf(src) })}
        {alt ? ` — ${alt}` : ''}
      </span>
      <button
        type="button"
        onClick={() => setLoad(true)}
        className="shrink-0 rounded-control border border-subtle px-2 py-0.5 text-ink hover:bg-raised"
        title={src}
      >
        {t('remoteImages.load')}
      </button>
    </span>
  )
}

/**
 * A remote image in trusted content (help guide, changelog): loads straight away, except while
 * the proxy rule blocks remote web content.
 */
export const TrustedRemoteImage: React.FC<
  React.ImgHTMLAttributes<HTMLImageElement> & { src: string }
> = ({ src, alt, ...props }) => {
  const web = useWebContentStatus()
  if (!web.allowed) return <WebContentBlocked reason={web.reason} compact />
  return <img src={src} alt={alt} {...props} />
}

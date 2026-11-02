import React, { forwardRef } from 'react'
import { DrawIoEmbed, DrawIoEmbedRef } from 'react-drawio'
import { useWebContentStatus } from '@renderer/lib/webContent'
import { WebContentBlocked } from './WebContentBlocked'

type Props = React.ComponentProps<typeof DrawIoEmbed>

/**
 * `DrawIoEmbed` (embed.diagrams.net in an iframe), or a notice while the proxy rule blocks remote
 * web content (see `lib/webContent.ts`).
 */
export const GatedDrawIoEmbed = forwardRef<DrawIoEmbedRef, Props>((props, ref) => {
  const web = useWebContentStatus()
  if (!web.allowed) return <WebContentBlocked reason={web.reason} />
  return <DrawIoEmbed ref={ref} {...props} />
})
GatedDrawIoEmbed.displayName = 'GatedDrawIoEmbed'

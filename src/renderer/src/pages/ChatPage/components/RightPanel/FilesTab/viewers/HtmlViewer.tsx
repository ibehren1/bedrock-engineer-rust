import React from 'react'
import { SandboxedHtmlPreview } from '../../../Code/HtmlBlock'

/**
 * An HTML file rendered from its text in the opaque-origin `htmlpreview://` sandbox, like the
 * chat's ```html blocks. Never loaded from the `workspacefile` scheme, which would run it with
 * the app's origin.
 */
export const HtmlViewer: React.FC<{ content: string }> = ({ content }) => (
  <div className="h-full bg-white">
    <SandboxedHtmlPreview html={content} />
  </div>
)

export default HtmlViewer

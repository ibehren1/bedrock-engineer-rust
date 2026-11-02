import React from 'react'
import { SandpackPreview } from '@codesandbox/sandpack-react'
import { FiMaximize } from 'react-icons/fi'
import { useWebContentStatus } from '@renderer/lib/webContent'
import { WebContentBlocked } from '@renderer/components/WebContentBlocked'

interface PreviewProps {
  isDark: boolean
  code: string
}

export const Preview: React.FC<PreviewProps> = ({ isDark, code }) => {
  // The preview runs on codesandbox.io; not while a proxy it wouldn't use is on.
  const web = useWebContentStatus()
  if (!web.allowed) return <WebContentBlocked reason={web.reason} />
  return (
    <SandpackPreview
      id="sandpack-preview"
      key={code}
      style={{
        height: '100%',
        borderRadius: '8px',
        border: isDark ? '2px solid black' : '2px solid white'
      }}
      showRestartButton={true}
      showOpenNewtab={true}
      showSandpackErrorOverlay={true}
      showOpenInCodeSandbox={false}
      showNavigator={true}
      actionsChildren={
        <button
          onClick={() => {
            const iframe = document.getElementById('sandpack-preview')
            if (iframe) {
              iframe?.requestFullscreen()
            }
          }}
          className="border rounded-full bg-[#EFEFEF] p-2 text-ink-muted hover:text-ink"
        >
          <FiMaximize className="text-gray" />
        </button>
      }
    />
  )
}

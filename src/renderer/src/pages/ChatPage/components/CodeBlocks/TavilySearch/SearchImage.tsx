import React from 'react'
import { useWebContentStatus } from '@renderer/lib/webContent'
import { WebContentBlocked } from '@renderer/components/WebContentBlocked'

interface Props {
  url: string
}

export const SearchImage: React.FC<Props> = ({ url }) => {
  const web = useWebContentStatus()
  const handleImageClick = () => {
    open(url)
  }
  if (!web.allowed) return <WebContentBlocked reason={web.reason} compact />

  return (
    <div
      className="border border-subtle rounded-container overflow-hidden cursor-pointer hover:opacity-90 transition-opacity"
      onClick={handleImageClick}
    >
      <img src={url} alt="Search result" className="w-full h-48 object-cover" loading="lazy" />
    </div>
  )
}

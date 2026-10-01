import React from 'react'

interface Props {
  url: string
}

export const SearchImage: React.FC<Props> = ({ url }) => {
  const handleImageClick = () => {
    open(url)
  }

  return (
    <div
      className="border border-subtle rounded-container overflow-hidden cursor-pointer hover:opacity-90 transition-opacity"
      onClick={handleImageClick}
    >
      <img src={url} alt="Search result" className="w-full h-48 object-cover" loading="lazy" />
    </div>
  )
}

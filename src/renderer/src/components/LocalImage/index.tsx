import React, { useEffect, useState } from 'react'

interface LocalImageProps {
  src: string
  alt: string
  className?: string
}

const LocalImage: React.FC<LocalImageProps> = ({ src, alt, className }) => {
  const [imageUrl, setImageUrl] = useState<string>('')
  const [error, setError] = useState<string>('')
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const loadImage = async () => {
      try {
        setLoading(true)
        const dataUrl = await window.api.images.getLocalImage(src)
        setImageUrl(dataUrl)
        setError('')
      } catch (err) {
        setError('Failed to load image')
        console.error('Error loading image:', err)
      } finally {
        setLoading(false)
      }
    }

    if (src) {
      loadImage()
    }
  }, [src])

  if (loading) {
    return <div className="animate-pulse bg-raised rounded-container w-full h-48"></div>
  }

  if (error) {
    return (
      <div className="text-danger text-sm p-2 border border-danger rounded-container">{error}</div>
    )
  }

  return (
    <img
      src={imageUrl}
      alt={alt}
      className={`max-w-full rounded-container ${className || ''}`}
      loading="lazy"
    />
  )
}

export default LocalImage

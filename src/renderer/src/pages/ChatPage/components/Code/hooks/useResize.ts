import { useState, useCallback, useRef, useEffect } from 'react'

type UseResizeProps = {
  initialHeight?: number
  minHeight?: number
  maxHeight?: number
}

type UseResizeReturn = {
  height: number
  isResizing: boolean
  handleMouseDown: (e: React.MouseEvent) => void
  containerRef: React.RefObject<HTMLDivElement>
}

export const useResize = ({
  initialHeight = 400,
  minHeight = 200,
  maxHeight = 800
}: UseResizeProps = {}): UseResizeReturn => {
  const [height, setHeight] = useState(initialHeight)
  const [isResizing, setIsResizing] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const startY = useRef(0)
  const startHeight = useRef(0)

  const handleMouseMove = useCallback(
    (e: MouseEvent) => {
      if (!isResizing) return

      const deltaY = e.clientY - startY.current
      const newHeight = Math.max(minHeight, Math.min(maxHeight, startHeight.current + deltaY))

      setHeight(newHeight)
    },
    [isResizing, minHeight, maxHeight]
  )

  const handleMouseUp = useCallback(() => {
    setIsResizing(false)
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
  }, [])

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      setIsResizing(true)
      startY.current = e.clientY
      startHeight.current = height

      document.body.style.cursor = 'ns-resize'
      document.body.style.userSelect = 'none'
    },
    [height]
  )

  useEffect(() => {
    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove)
      document.addEventListener('mouseup', handleMouseUp)

      return () => {
        document.removeEventListener('mousemove', handleMouseMove)
        document.removeEventListener('mouseup', handleMouseUp)
      }
    }
    return undefined
  }, [isResizing, handleMouseMove, handleMouseUp])

  return {
    height,
    isResizing,
    handleMouseDown,
    containerRef
  }
}

import React from 'react'
import { VscGripper } from 'react-icons/vsc'
import { useResize } from './hooks/useResize'

type ResizableContainerProps = {
  children: React.ReactNode
  initialHeight?: number
  minHeight?: number
  maxHeight?: number
  className?: string
}

export const ResizableContainer: React.FC<ResizableContainerProps> = ({
  children,
  initialHeight = 800,
  minHeight = 200,
  maxHeight = 1800,
  className = ''
}) => {
  const { height, isResizing, handleMouseDown, containerRef } = useResize({
    initialHeight,
    minHeight,
    maxHeight
  })

  return (
    <div ref={containerRef} className={`relative ${className}`}>
      {/* Content area with dynamic height */}
      <div style={{ height: `${height}px` }} className="overflow-hidden h-full">
        {children}
      </div>

      {/* Resize handle */}
      <div
        className={`flex items-center justify-center h-3 bg-raised border-t border-strong cursor-ns-resize hover:bg-sunken transition-colors ${
          isResizing ? 'bg-accent-tint' : ''
        }`}
        onMouseDown={handleMouseDown}
        title="Drag to resize"
      >
        <VscGripper size={12} className={`text-ink-faint ${isResizing ? 'text-accent' : ''}`} />
      </div>

      {/* Height indicator - shown during resize */}
      {isResizing && (
        <div className="absolute right-2 bottom-4 bg-ink text-canvas text-xs px-2 py-1 rounded-control pointer-events-none z-10">
          {height}px
        </div>
      )}
    </div>
  )
}

export default ResizableContainer

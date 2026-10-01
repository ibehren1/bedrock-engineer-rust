import { useRef, useEffect } from 'react'

/**
 * Auto-scroll hook that automatically scrolls to the bottom of a container
 * when dependencies change
 */
export const useAutoScroll = (dependencies: unknown[]) => {
  const chatContainerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (chatContainerRef.current) {
      chatContainerRef.current.scrollTop = chatContainerRef.current.scrollHeight
    }
  }, dependencies)

  return { chatContainerRef }
}

import React, { useEffect, useReducer } from 'react'
import { TbRobot } from 'react-icons/tb'
import { AGENT_ICONS } from './AgentIcons'
import {
  getIconData,
  IconData,
  isIconCollectionLoaded,
  loadIconCollection,
  parseIconValue,
  subscribeToIconCollections
} from './iconCollections'

type AgentIconViewProps = {
  /** Curated value (`robot`) or Iconify id (`tabler:rocket`). */
  icon?: string
  iconColor?: string
  className?: string
  style?: React.CSSProperties
  /** Rendered while a collection loads, and when the value resolves to nothing. */
  fallback?: React.ReactElement
}

/**
 * Renders an agent's icon wherever it comes from. Iconify collections live in
 * lazily loaded chunks, so the first render of one of their icons shows the
 * placeholder and swaps in once the collection is in memory.
 */
export const AgentIconView: React.FC<AgentIconViewProps> = ({
  icon,
  iconColor,
  className,
  style,
  fallback
}) => {
  const iconRef = parseIconValue(icon)
  const collectionId = iconRef?.collection
  const [, rerender] = useReducer((count: number) => count + 1, 0)

  useEffect(() => {
    if (!collectionId || isIconCollectionLoaded(collectionId)) return
    // Re-render once the chunk lands; the cache is shared, so several icons from
    // the same collection share one load.
    const unsubscribe = subscribeToIconCollections(rerender)
    void loadIconCollection(collectionId).catch(() => undefined)
    return unsubscribe
  }, [collectionId])

  const mergedStyle = iconColor ? { color: iconColor, ...style } : style

  if (iconRef) {
    const data = getIconData(icon)
    if (data) return <IconifyIcon data={data} className={className} style={mergedStyle} />
  } else {
    const curated = AGENT_ICONS.find((option) => option.value === icon)
    if (curated) {
      return React.cloneElement(curated.icon as React.ReactElement, {
        className,
        style: mergedStyle
      })
    }
  }

  const placeholder = fallback ?? <TbRobot />
  return React.cloneElement(placeholder, {
    className: placeholder.props.className ?? className,
    style: placeholder.props.style ?? mergedStyle
  })
}

type IconifyIconProps = {
  data: IconData
  className?: string
  style?: React.CSSProperties
}

/**
 * Iconify bodies are SVG fragments that colour themselves with `currentColor`,
 * matching how react-icons behaves, so the same className/style props work for
 * both kinds of icon.
 */
export const IconifyIcon: React.FC<IconifyIconProps> = ({ data, className, style }) => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox={data.viewBox}
    width="1em"
    height="1em"
    aria-hidden="true"
    focusable="false"
    className={className}
    style={style}
    dangerouslySetInnerHTML={{ __html: data.body }}
  />
)

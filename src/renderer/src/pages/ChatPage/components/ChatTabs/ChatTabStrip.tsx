import React from 'react'
import type { ChatTab } from './chatTabs'

export type ChatTabItem = {
  key: ChatTab
  label: string
  icon: React.ReactNode
  /** Shown after the label: a count, an attention dot, the running cost. */
  adornment?: React.ReactNode
  title?: string
}

type ChatTabStripProps = {
  items: ChatTabItem[]
  active: ChatTab
  onChange: (tab: ChatTab) => void
}

/** The tab row across the top of the chat column. */
export const ChatTabStrip: React.FC<ChatTabStripProps> = ({ items, active, onChange }) => (
  <div
    role="tablist"
    className="flex items-center gap-0.5 px-1 border-b border-subtle shrink-0 min-w-0"
  >
    {items.map((item) => (
      <button
        key={item.key}
        role="tab"
        aria-selected={active === item.key}
        onClick={() => onChange(item.key)}
        title={item.title}
        className={`inline-flex items-center gap-1.5 px-2.5 py-2 text-xs -mb-px border-b-2 whitespace-nowrap ${
          active === item.key
            ? 'text-ink border-accent'
            : 'text-ink-faint border-transparent hover:text-ink'
        }`}
      >
        {item.icon}
        {item.label}
        {item.adornment}
      </button>
    ))}
  </div>
)

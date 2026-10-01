import React, { useState, useEffect, useRef } from 'react'
import { EllipsisVerticalIcon } from '@heroicons/react/24/outline'

interface ActionMenuItem {
  key: string
  label: string
  icon?: React.ReactNode
  onClick: () => void
  variant?: 'default' | 'danger'
  separator?: boolean
}

interface ActionMenuProps {
  items: ActionMenuItem[]
  title?: string
}

export const ActionMenu: React.FC<ActionMenuProps> = ({ items, title = 'More actions' }) => {
  const [showMenu, setShowMenu] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setShowMenu(false)
      }
    }

    if (showMenu) {
      document.addEventListener('mousedown', handleClickOutside)
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
    }
  }, [showMenu])

  const handleItemClick = (item: ActionMenuItem) => {
    item.onClick()
    setShowMenu(false)
  }

  return (
    <div className="relative" ref={menuRef}>
      <button
        onClick={() => setShowMenu(!showMenu)}
        className="p-2 text-ink-muted hover:text-ink transition-colors rounded-control hover:bg-raised"
        title={title}
      >
        <EllipsisVerticalIcon className="h-4 w-4" />
      </button>

      {showMenu && (
        <div className="absolute right-0 top-full mt-2 w-48 bg-surface rounded-control shadow-lg border border-subtle z-10">
          <div className="py-1">
            {items.map((item, index) => (
              <React.Fragment key={item.key}>
                {item.separator && index > 0 && <hr className="my-1 border-subtle" />}
                <button
                  onClick={() => handleItemClick(item)}
                  className={`flex items-center w-full px-2.5 py-1 text-sm transition-colors ${
                    item.variant === 'danger'
                      ? 'text-danger hover:bg-danger-soft'
                      : 'text-ink hover:bg-raised'
                  }`}
                >
                  {item.icon && <span className="mr-3">{item.icon}</span>}
                  {item.label}
                </button>
              </React.Fragment>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

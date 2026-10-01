import React from 'react'
import { IconType } from 'react-icons'

interface SettingSectionProps {
  title: string
  description?: string
  icon?: IconType
  children: React.ReactNode
}

export const SettingSection: React.FC<SettingSectionProps> = ({
  title,
  description,
  icon: Icon,
  children
}) => {
  return (
    <section className="flex flex-col gap-2">
      <div>
        <h2 className="text-heading text-ink">
          {Icon && (
            <div className="flex gap-2 items-center">
              <Icon className="text-base" />
              <span>{title}</span>
            </div>
          )}
          {!Icon && title}
        </h2>
        {description && <p className="mt-1 text-sm text-ink-muted">{description}</p>}
      </div>
      <div className="space-y-2">{children}</div>
    </section>
  )
}

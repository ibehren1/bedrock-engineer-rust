import React, { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useSettings } from '@renderer/contexts/SettingsContext'

type PlanActToggleProps = {
  className?: string
}

const planModeStyle = 'bg-warning-soft text-ink'
// Act mode is the default, so this pill is on screen for the whole session. It
// used to animate its gradient position, which repaints continuously and shows
// up as steady battery drain while the app just sits there.
const actModeStyle = 'bg-accent text-accent-fg'
const unselectedStyle = 'bg-surface-2 text-ink-muted hover:text-ink'

export const PlanActToggle: React.FC<PlanActToggleProps> = ({ className = '' }) => {
  const { t } = useTranslation()
  const { planMode, setPlanMode } = useSettings()

  // プラットフォームに応じた翻訳キーを決定
  const { planModeTooltipKey, actModeTooltipKey } = useMemo(() => {
    const isMac = navigator.platform.toLowerCase().includes('mac')
    return {
      planModeTooltipKey: isMac
        ? 'Plan mode - Read-only tools enabled (⌘+Shift+A)'
        : 'Plan mode - Read-only tools enabled (Ctrl+Shift+A)',
      actModeTooltipKey: isMac
        ? 'Act mode - All tools enabled (⌘+Shift+A)'
        : 'Act mode - All tools enabled (Ctrl+Shift+A)'
    }
  }, [])

  return (
    <div className={`flex rounded-full border border-accent-soft overflow-hidden ${className}`}>
      <button
        title={t(planModeTooltipKey)}
        className={`px-3 py-1.5 text-xs font-medium transition-colors ${
          planMode ? planModeStyle : unselectedStyle
        }`}
        onClick={() => setPlanMode(true)}
        aria-pressed={planMode}
      >
        Plan
      </button>
      <button
        title={t(actModeTooltipKey)}
        className={`px-3 py-1.5 text-xs font-medium transition-colors ${
          !planMode ? actModeStyle : unselectedStyle
        }`}
        onClick={() => setPlanMode(false)}
        aria-pressed={!planMode}
      >
        Act
      </button>
    </div>
  )
}

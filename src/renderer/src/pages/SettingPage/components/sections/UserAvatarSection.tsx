import React, { useState, useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { FcBusinessman } from 'react-icons/fc'
import { LiaUserCircleSolid } from 'react-icons/lia'
import EmojiPicker, { EmojiStyle, Theme, type EmojiClickData } from 'emoji-picker-react'
import { SettingSection } from '../SettingSection'
import useSetting from '@renderer/hooks/useSetting'
import { useTheme } from '@renderer/hooks/useTheme'

export const UserAvatarSection: React.FC = () => {
  const { t } = useTranslation()
  const { userEmoji, setUserEmoji, userName, setUserName } = useSetting()
  const { isDarkMode } = useTheme()
  const [isPickerOpen, setIsPickerOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  // Close the picker when clicking outside of it.
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsPickerOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const handleEmojiClick = (emojiData: EmojiClickData) => {
    setUserEmoji(emojiData.emoji)
    setIsPickerOpen(false)
  }

  return (
    <SettingSection
      title={t('userAvatar.title')}
      description={t('userAvatar.description')}
      icon={FcBusinessman}
    >
      <div className="flex items-center gap-3" ref={containerRef}>
        <div className="relative">
          <button
            type="button"
            onClick={() => setIsPickerOpen((open) => !open)}
            className="flex items-center justify-center w-12 h-12 rounded-full border border-strong
              border-subtle bg-surface hover:border-accent
              transition-colors"
            aria-label={t('userAvatar.choose')}
          >
            {userEmoji ? (
              <span className="text-2xl leading-none" role="img" aria-label="user avatar">
                {userEmoji}
              </span>
            ) : (
              <LiaUserCircleSolid className="h-7 w-7 text-ink-muted" />
            )}
          </button>

          {isPickerOpen && (
            <div className="absolute z-30 mt-2 left-0">
              <EmojiPicker
                onEmojiClick={handleEmojiClick}
                theme={isDarkMode ? Theme.DARK : Theme.LIGHT}
                emojiStyle={EmojiStyle.NATIVE}
                lazyLoadEmojis
                width={320}
                height={400}
              />
            </div>
          )}
        </div>

        <div className="flex flex-col gap-1">
          <span className="text-sm text-ink">
            {userEmoji ? t('userAvatar.current') : t('userAvatar.usingDefault')}
          </span>
          {userEmoji && (
            <button
              type="button"
              onClick={() => setUserEmoji('')}
              className="text-xs text-accent hover:underline text-left"
            >
              {t('userAvatar.reset')}
            </button>
          )}
        </div>
      </div>

      <div className="flex flex-col gap-1 mt-4">
        <label htmlFor="user-name-input" className="text-sm font-medium text-ink">
          {t('userName.title')}
        </label>
        <p className="text-xs text-ink-muted">{t('userName.description')}</p>
        <input
          id="user-name-input"
          type="text"
          value={userName}
          onChange={(e) => setUserName(e.target.value)}
          placeholder={t('userName.placeholder')}
          className="mt-1 w-full max-w-sm rounded-container border border-strong
            bg-surface px-3 py-2 text-sm text-ink
            focus:border-accent focus:ring-accent"
        />
      </div>
    </SettingSection>
  )
}

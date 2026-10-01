import React from 'react'
import {
  AppearanceSection,
  LanguageSection,
  NotificationSection,
  SidebarSection,
  UserAvatarSection
} from '../sections'

export const GeneralTab: React.FC = () => (
  <div className="flex flex-col gap-4">
    <LanguageSection />
    <AppearanceSection />
    <UserAvatarSection />
    <SidebarSection />
    <NotificationSection />
  </div>
)

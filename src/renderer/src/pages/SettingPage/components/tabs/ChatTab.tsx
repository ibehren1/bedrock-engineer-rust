import React from 'react'
import {
  AdvancedSection,
  AgentChatSection,
  BackgroundAgentsSection,
  RemoteImagesSection
} from '../sections'

export const ChatTab: React.FC = () => (
  <div className="flex flex-col gap-4">
    <AgentChatSection />
    <RemoteImagesSection />
    <BackgroundAgentsSection />
    <AdvancedSection />
  </div>
)

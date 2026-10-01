import React from 'react'
import { AdvancedSection, AgentChatSection } from '../sections'

export const ChatTab: React.FC = () => (
  <div className="flex flex-col gap-4">
    <AgentChatSection />
    <AdvancedSection />
  </div>
)

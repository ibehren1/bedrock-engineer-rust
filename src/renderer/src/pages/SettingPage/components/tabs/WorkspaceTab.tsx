import React from 'react'
import {
  AgentFoldersSection,
  AllowedCommandsSection,
  ConfigDirSection,
  NetworkAccessSection,
  ProjectSection
} from '../sections'

export const WorkspaceTab: React.FC = () => (
  <div className="flex flex-col gap-4">
    <ProjectSection />
    <AgentFoldersSection />
    <AllowedCommandsSection />
    <NetworkAccessSection />
    <ConfigDirSection />
  </div>
)

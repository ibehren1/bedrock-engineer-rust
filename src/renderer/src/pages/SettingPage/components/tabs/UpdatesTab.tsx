import React from 'react'
import { ChangelogSection, UpdatesSection } from '../sections'

export const UpdatesTab: React.FC = () => (
  <div className="flex flex-col gap-4">
    <UpdatesSection />
    <ChangelogSection />
  </div>
)

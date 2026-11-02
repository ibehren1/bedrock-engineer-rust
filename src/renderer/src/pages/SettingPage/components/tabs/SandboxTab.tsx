import React from 'react'
import {
  SandboxEnvironmentSection,
  SandboxImageSection,
  SandboxLimitsSection,
  SandboxRestrictionsSection
} from '../sections'

export const SandboxTab: React.FC = () => (
  <div className="flex flex-col gap-4">
    <SandboxEnvironmentSection />
    <SandboxImageSection />
    <SandboxLimitsSection />
    <SandboxRestrictionsSection />
  </div>
)

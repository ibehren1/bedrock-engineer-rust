import React from 'react'
import {
  BedrockModelsSection,
  DefaultModelsSection,
  GuardrailSection,
  InferenceParametersSection,
  ThinkingModeSection
} from '../sections'

export const ModelsTab: React.FC = () => (
  <div className="flex flex-col gap-4">
    <DefaultModelsSection />
    <ThinkingModeSection />
    <BedrockModelsSection />
    <InferenceParametersSection />
    <GuardrailSection />
  </div>
)

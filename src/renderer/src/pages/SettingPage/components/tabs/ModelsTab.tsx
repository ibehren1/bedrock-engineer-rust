import React from 'react'
import {
  BedrockModelsSection,
  GuardrailSection,
  InferenceParametersSection,
  LightModelSection
} from '../sections'

export const ModelsTab: React.FC = () => (
  <div className="flex flex-col gap-4">
    <BedrockModelsSection />
    <InferenceParametersSection />
    <LightModelSection />
    <GuardrailSection />
  </div>
)

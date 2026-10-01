import LocalImage from '@renderer/components/LocalImage'
import React from 'react'

interface GenerateImageResult {
  success: boolean
  name: string
  message: string
  result: {
    imagePath: string
    modelUsed: string
    seed?: number
    prompt: string
    negativePrompt?: string
    aspect_ratio: string
  }
}

export const GenerateImageResult: React.FC<{ response: GenerateImageResult }> = ({ response }) => {
  const { result } = response
  const imageUrl = `${result.imagePath}`

  return (
    <div className="flex gap-3 bg-surface-2 text-ink p-2.5 rounded-container overflow-hidden shadow-raised border border-subtle">
      {/* Image Display */}

      <LocalImage src={imageUrl} alt={result.prompt} className="aspect-auto h-[30vh]" />

      {/* Image Details */}
      <div className="space-y-3 text-sm text-ink-faint h-[30vh] overflow-y-scroll">
        {/* Prompt Section */}
        <div className="space-y-2">
          <div className="flex flex-col">
            <span className="font-semibold mb-1">Prompt:</span>
            <span className="font-mono bg-sunken p-2 rounded-control">{result.prompt}</span>
          </div>

          {result.negativePrompt && (
            <div className="flex flex-col">
              <span className="font-semibold mb-1">Negative Prompt:</span>
              <span className="font-mono bg-sunken p-2 rounded-control text-danger">
                {result.negativePrompt}
              </span>
            </div>
          )}
        </div>

        {/* Technical Details */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2 bg-sunken p-2 rounded-control">
          <div className="flex items-center">
            <span className="font-semibold mr-2">Model:</span>
            <span className="font-mono">{result.modelUsed}</span>
          </div>

          <div className="flex items-center">
            <span className="font-semibold mr-2">Aspect Ratio:</span>
            <span className="font-mono">{result.aspect_ratio}</span>
          </div>

          {result.seed !== undefined && (
            <div className="flex items-center">
              <span className="font-semibold mr-2">Seed:</span>
              <span className="font-mono">{result.seed}</span>
            </div>
          )}

          <div className="flex items-center">
            <span className="font-semibold mr-2">Path:</span>
            <span className="font-mono text-xs truncate" title={result.imagePath}>
              {result.imagePath}
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}

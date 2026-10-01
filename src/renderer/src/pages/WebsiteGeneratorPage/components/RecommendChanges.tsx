import React from 'react'
import { motion } from 'framer-motion'
import LoadingDotsLottie from '../LoadingDots.lottie'

interface Recommendation {
  title: string
  value: string
}

interface RecommendChangesProps {
  loading: boolean
  recommendations: Recommendation[]
  onSelect: (value: string) => void
  loadingText: string
}

export const RecommendChanges: React.FC<RecommendChangesProps> = ({
  loading,
  recommendations,
  onSelect,
  loadingText
}) => {
  if (loading) {
    return (
      <div className="flex gap-1 justify-start items-center text-ink">
        <LoadingDotsLottie className="h-[2rem]" />
        <span className="text-ink">{loadingText}</span>
      </div>
    )
  }

  return (
    <div className="flex flex-nowrap gap-2 min-w-0 whitespace-nowrap">
      {recommendations?.map((recommendation, index) => (
        <motion.button
          initial={{ opacity: 0, scale: 0 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: index * 0.2 }}
          key={recommendation.title}
          className="cursor-pointer rounded-full border p-2 text-xs hover:border-strong hover:bg-surface-2 whitespace-nowrap flex-shrink-0"
          onClick={() => onSelect(recommendation.value)}
        >
          {recommendation.title}
        </motion.button>
      ))}
    </div>
  )
}

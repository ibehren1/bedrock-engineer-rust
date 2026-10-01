import Lottie, { LottieComponentProps } from 'lottie-react'
import data from '../../assets/lottie/loading-voice-with-ai.json'

// ignore type animationData from LottieComponentProps
type LoadingDotsLottieProps = Omit<LottieComponentProps, 'animationData'>

const VoiceAILottie = (props?: LoadingDotsLottieProps) => {
  return <Lottie {...props} animationData={data} />
}

export default VoiceAILottie

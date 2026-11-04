import { Lottie } from 'lottie-react'
import data from '../../assets/lottie/loading-voice-with-ai.json'

type VoiceAILottieProps = { className?: string }

// lottie-react 3 defaults autoplay and loop to off; these spinners play continuously. Its
// display fills its container (width and height 100%), so the container is sized here: the
// caller's width or height, the other side from the animation's own aspect ratio.
const VoiceAILottie = ({ className }: VoiceAILottieProps) => {
  return (
    <div className={className} style={{ aspectRatio: `${data.w} / ${data.h}` }}>
      <Lottie src={data} autoplay loop />
    </div>
  )
}

export default VoiceAILottie

import LazyVisibleMessage from '../LazyVisibleMessage'
import LoadingDataBaseLottie from '../LoadingDataBase.lottie'

export const RagLoader = () => {
  return (
    <div className="flex flex-col justify-center items-center gap-2">
      <LoadingDataBaseLottie className="w-[6rem]" />
      <span className="text-sm text-ink-faint">Connecting datasource...</span>
      <span className="text-xs text-ink-faint">
        <LazyVisibleMessage message="Searching for related source code" />
      </span>
    </div>
  )
}

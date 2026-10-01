import LazyVisibleMessage from '../pages/WebsiteGeneratorPage/LazyVisibleMessage'
import LoadingWebsiteLottie from '../pages/WebsiteGeneratorPage/LoadingWebsite.lottie'

export const WebLoader = () => {
  return (
    <div className="flex flex-col justify-center items-center gap-2">
      <LoadingWebsiteLottie className="w-[8rem]" />
      <span className="text-sm text-ink-faint">Searching websites...</span>
      <span className="text-xs text-ink-faint">
        <LazyVisibleMessage message="Searching for related source code" />
      </span>
    </div>
  )
}

type LoaderProps = {
  text?: string
}

export const Loader = (props: LoaderProps) => {
  return (
    <div className="flex flex-col justify-center items-center gap-2">
      <div className="animate-spin h-8 w-8 bg-accent-tint rounded-container"></div>
      <span className="text-sm text-ink-faint">{props?.text || 'loading...'}</span>
    </div>
  )
}

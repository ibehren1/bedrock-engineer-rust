/* eslint-disable react/prop-types */
import { BsDatabase, BsDatabaseCheck } from 'react-icons/bs'

type KnowledgeBaseConnectButtonProps = {
  enableKnowledgeBase: boolean
  handleOpenDataSourceConnectModal: () => void
}

export const KnowledgeBaseConnectButton: React.FC<KnowledgeBaseConnectButtonProps> = (props) => {
  const { enableKnowledgeBase, handleOpenDataSourceConnectModal } = props
  return (
    <button
      onClick={() => handleOpenDataSourceConnectModal()}
      className={`flex items-center justify-center p-[2px] overflow-hidden text-xs text-ink rounded-container group
        ${
          enableKnowledgeBase
            ? 'bg-gradient-to-br from-danger via-danger to-warning group-hover:from-danger group-hover:via-danger group-hover:to-warning'
            : 'border border-subtle'
        }
        text-ink hover:text-ink focus:ring-4 focus:outline-none focus:ring-danger`}
    >
      <span
        className={`items-center px-3 py-1.5 transition-all ease-in duration-75 rounded-control flex gap-2
          ${enableKnowledgeBase ? 'bg-surface group-hover:bg-opacity-0' : 'bg-transparent'}`}
      >
        {enableKnowledgeBase ? (
          <BsDatabaseCheck className="text-sm" />
        ) : (
          <BsDatabase className="text-sm" />
        )}
        {enableKnowledgeBase ? 'Connected' : 'Connect'}
      </span>
    </button>
  )
}

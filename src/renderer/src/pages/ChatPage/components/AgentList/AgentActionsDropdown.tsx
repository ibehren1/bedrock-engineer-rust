import React from 'react'
import { CustomAgent } from '@/types/agent-chat'
import { FiMoreVertical } from 'react-icons/fi'
import { Dropdown, DropdownItem } from 'flowbite-react'
import { useTranslation } from 'react-i18next'
import { PROTECTED_DEFAULT_AGENT_IDS } from './useAgentFilter'

interface AgentActionsDropdownProps {
  agent: CustomAgent
  onEdit?: (agent: CustomAgent) => void
  onDuplicate?: (agent: CustomAgent) => void
  onDelete?: (agentId: string) => void
  onSaveAsShared?: (agent: CustomAgent) => void
  onDeleteSharedFile?: (agent: CustomAgent) => void
  onDownloadYaml?: (agent: CustomAgent) => void
  onShareToOrganization?: (agent: CustomAgent) => void
  onConvertToStrands?: (agentId: string) => void
}

export const AgentActionsDropdown: React.FC<AgentActionsDropdownProps> = ({
  agent,
  onEdit,
  onDuplicate,
  onDelete,
  onSaveAsShared,
  onDeleteSharedFile,
  onDownloadYaml,
  onShareToOrganization,
  onConvertToStrands
}) => {
  const { t } = useTranslation()

  const isCustomAgent = agent.isCustom ?? true
  const isEditable = isCustomAgent && !agent.isShared

  // 共有エージェント（ファイル管理）と一覧に出ないデフォルトエージェントは
  // 削除・非表示にできない。カスタムエージェントは削除、デフォルトは非表示になる。
  const isRemovable =
    !agent.isShared && !!agent.id && !PROTECTED_DEFAULT_AGENT_IDS.includes(agent.id)

  // Only agents loaded from a project file have a file to remove; organization agents are also
  // flagged as shared but live in S3, so they carry no path.
  const hasSharedFile = !!agent.isShared && !!agent.sharedFilePath

  // メニュー項目が1つもない場合は表示しない
  const hasAnyAction =
    (isEditable && onEdit) ||
    onDuplicate ||
    onDownloadYaml ||
    onConvertToStrands ||
    (!agent.isShared && onSaveAsShared) ||
    (isEditable && onShareToOrganization) ||
    (hasSharedFile && onDeleteSharedFile) ||
    (isRemovable && onDelete)

  if (!hasAnyAction) {
    return null
  }

  return (
    <div onClick={(e) => e.stopPropagation()}>
      <Dropdown
        label=""
        dismissOnClick={true}
        renderTrigger={() => (
          <button className="p-1 text-ink-muted hover:text-ink rounded-container hover:bg-raised">
            <FiMoreVertical className="w-4 h-4" />
          </button>
        )}
      >
        {isEditable && onEdit && (
          <DropdownItem onClick={() => onEdit(agent)} className="w-48">
            {t('edit')}
          </DropdownItem>
        )}
        {onDuplicate && (
          <DropdownItem onClick={() => onDuplicate(agent)} className="w-48">
            {t('duplicate')}
          </DropdownItem>
        )}
        {onDownloadYaml && (
          <DropdownItem onClick={() => onDownloadYaml(agent)} className="w-48">
            {t('downloadYaml')}
          </DropdownItem>
        )}
        {onConvertToStrands && (
          <DropdownItem onClick={() => onConvertToStrands(agent.id!)} className="w-48">
            {t('convertToStrands')}
          </DropdownItem>
        )}
        {!agent.isShared && onSaveAsShared && (
          <DropdownItem onClick={() => onSaveAsShared(agent)} className="w-48">
            {t('saveAsShared')}
          </DropdownItem>
        )}
        {isEditable && onShareToOrganization && (
          <DropdownItem onClick={() => onShareToOrganization(agent)} className="w-48">
            {t('shareToOrganization')}
          </DropdownItem>
        )}
        {hasSharedFile && onDeleteSharedFile && (
          <DropdownItem onClick={() => onDeleteSharedFile(agent)} className="text-danger w-48">
            {t('deleteSharedFile')}
          </DropdownItem>
        )}
        {isRemovable && onDelete && (
          <DropdownItem
            onClick={() => onDelete(agent.id!)}
            className={isCustomAgent ? 'text-danger w-48' : 'w-48'}
          >
            {isCustomAgent ? t('delete') : t('myAgents.hideDefault')}
          </DropdownItem>
        )}
      </Dropdown>
    </div>
  )
}

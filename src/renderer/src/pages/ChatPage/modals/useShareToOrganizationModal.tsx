import React, { useState } from 'react'
import { Modal, Button } from 'flowbite-react'
import { useTranslation } from 'react-i18next'
import { HiOfficeBuilding } from 'react-icons/hi'
import { CustomAgent, OrganizationConfig } from '@/types/agent-chat'
import { useSettings } from '@renderer/contexts/SettingsContext'

interface ShareToOrganizationModalProps {
  agent?: CustomAgent
  isOpen: boolean
  onClose: () => void
  onShare: (agent: CustomAgent, organization: OrganizationConfig) => Promise<void>
}

const ShareToOrganizationModal: React.FC<ShareToOrganizationModalProps> = ({
  agent,
  isOpen,
  onClose,
  onShare
}) => {
  const { t } = useTranslation()
  const { organizations } = useSettings()
  const [selectedOrgId, setSelectedOrgId] = useState<string>('')
  const [isSharing, setIsSharing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleShare = async () => {
    if (!agent || !selectedOrgId) {
      setError(t('pleaseSelectOrganization', 'Please select an organization'))
      return
    }

    const selectedOrg = organizations.find((org) => org.id === selectedOrgId)
    if (!selectedOrg) {
      setError(t('organizationNotFound', 'Organization not found'))
      return
    }

    setIsSharing(true)
    setError(null)

    try {
      await onShare(agent, selectedOrg)
      onClose()
      setSelectedOrgId('')
    } catch (err) {
      setError(err instanceof Error ? err.message : t('unknownError', 'Unknown error occurred'))
    } finally {
      setIsSharing(false)
    }
  }

  const handleClose = () => {
    if (!isSharing) {
      setSelectedOrgId('')
      setError(null)
      onClose()
    }
  }

  if (!agent) return null

  return (
    <Modal show={isOpen} onClose={handleClose} size="md" className="bg-canvas">
      <div className="border-[0.5px] border-surface rounded-container shadow-xl">
        <Modal.Header className="border-b border-subtle bg-canvas rounded-t-container">
          {t('shareAgentToOrganization', 'Share Agent to Organization')}
        </Modal.Header>
        <Modal.Body className="p-0 bg-surface">
          <div className="space-y-2 p-3">
            {/* エージェント情報 */}
            <div className="p-3 bg-surface-2 rounded-container">
              <h4 className="font-medium text-ink mb-1">{t('agentToShare', 'Agent to Share')}</h4>
              <p className="text-sm text-ink-muted">
                <strong>{agent.name}</strong>
                {agent.description && ` - ${agent.description}`}
              </p>
            </div>

            {/* エラー表示 */}
            {error && (
              <div className="p-3 bg-danger-soft border border-danger rounded-control">
                <p className="text-danger text-sm">{error}</p>
              </div>
            )}

            {/* 組織選択 */}
            <div>
              <label className="block text-sm font-medium text-ink mb-2">
                {t('selectOrganization', 'Select Organization')}
              </label>

              {organizations.length === 0 ? (
                <div className="p-2.5 text-center text-ink-muted">
                  <HiOfficeBuilding className="w-8 h-8 mx-auto mb-2 opacity-50" />
                  <p className="text-sm">
                    {t('noOrganizationsConfigured', 'No organizations configured')}
                  </p>
                  <p className="text-xs mt-1">
                    {t(
                      'configureOrganizationsInDirectory',
                      'Configure organizations in Agent Directory'
                    )}
                  </p>
                </div>
              ) : (
                <div className="space-y-2 max-h-60 overflow-y-auto">
                  {organizations.map((org) => (
                    <label
                      key={org.id}
                      className={`flex items-center p-3 border rounded-container cursor-pointer transition-colors
                        ${
                          selectedOrgId === org.id
                            ? 'border-accent bg-accent-tint'
                            : 'border-strong hover:bg-surface-2'
                        }`}
                    >
                      <input
                        type="radio"
                        name="organization"
                        value={org.id}
                        checked={selectedOrgId === org.id}
                        onChange={(e) => setSelectedOrgId(e.target.value)}
                        className="sr-only"
                        disabled={isSharing}
                      />
                      <HiOfficeBuilding className="w-4 h-4 mr-3 text-ink-faint" />
                      <div className="flex-1">
                        <div className="font-medium text-ink">{org.name}</div>
                        {org.description && (
                          <div className="text-sm text-ink-muted">{org.description}</div>
                        )}
                        <div className="text-xs text-ink-faint mt-1">
                          S3: {org.s3Config.bucket}
                          {org.s3Config.prefix && `/${org.s3Config.prefix}`}
                        </div>
                      </div>
                      {selectedOrgId === org.id && (
                        <div className="w-4 h-4 bg-accent-tint rounded-full flex items-center justify-center">
                          <div className="w-2 h-2 bg-surface rounded-full"></div>
                        </div>
                      )}
                    </label>
                  ))}
                </div>
              )}
            </div>

            {/* 注意事項 */}
            {organizations.length > 0 && (
              <div className="p-3 bg-warning-soft border border-warning rounded-control">
                <p className="text-warning text-xs">
                  <strong>{t('note', 'Note')}:</strong>{' '}
                  {t(
                    'shareToOrganizationNote',
                    "The agent configuration will be uploaded to the selected organization's S3 bucket. Make sure you have proper permissions."
                  )}
                </p>
              </div>
            )}
          </div>
        </Modal.Body>
        <Modal.Footer className="border-t border-subtle bg-canvas rounded-b-container">
          <Button
            onClick={handleShare}
            disabled={isSharing || !selectedOrgId || organizations.length === 0}
            color="blue"
          >
            {isSharing ? t('sharing', 'Sharing...') : t('share', 'Share')}
          </Button>
          <Button color="gray" onClick={handleClose} disabled={isSharing}>
            {t('cancel', 'Cancel')}
          </Button>
        </Modal.Footer>
      </div>
    </Modal>
  )
}

export const useShareToOrganizationModal = () => {
  const [isOpen, setIsOpen] = useState(false)
  const [selectedAgent, setSelectedAgent] = useState<CustomAgent | undefined>(undefined)

  const openModal = (agent: CustomAgent) => {
    setSelectedAgent(agent)
    setIsOpen(true)
  }

  const closeModal = () => {
    setIsOpen(false)
    setSelectedAgent(undefined)
  }

  const handleShare = async (agent: CustomAgent, organization: OrganizationConfig) => {
    try {
      const result = await window.file.saveAgentToOrganization(agent, organization, {
        format: 'yaml'
      })
      if (!result.success) {
        throw new Error(result.error || 'Failed to share agent to organization')
      }
    } catch (error) {
      console.error('Error sharing agent to organization:', error)
      throw error
    }
  }

  const ShareToOrganizationModalComponent = () => (
    <ShareToOrganizationModal
      agent={selectedAgent}
      isOpen={isOpen}
      onClose={closeModal}
      onShare={handleShare}
    />
  )

  return {
    ShareToOrganizationModal: ShareToOrganizationModalComponent,
    openModal,
    closeModal
  }
}

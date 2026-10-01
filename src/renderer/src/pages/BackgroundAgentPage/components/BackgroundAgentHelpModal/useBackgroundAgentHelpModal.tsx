import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { RiCloseLine } from 'react-icons/ri'
import { FiCode, FiCpu, FiZap } from 'react-icons/fi'
import { Button } from '@renderer/components/ui'

export const useBackgroundAgentHelpModal = () => {
  const [isOpen, setIsOpen] = useState(false)
  const { t } = useTranslation()

  const openModal = () => setIsOpen(true)
  const closeModal = () => setIsOpen(false)

  const BackgroundAgentHelpModal = () => {
    // Handle ESC key press to close the modal
    const handleKeyDown = (event: React.KeyboardEvent) => {
      if (event.key === 'Escape') {
        closeModal()
      }
    }

    return isOpen ? (
      <div className="fixed inset-0 z-50 overflow-y-auto" onKeyDown={handleKeyDown}>
        {/* Backdrop */}
        <div className="fixed inset-0 bg-black bg-opacity-50" onClick={closeModal}></div>

        {/* Modal */}
        <div className="flex items-center justify-center min-h-screen p-2.5">
          <div
            className="relative w-full max-w-4xl bg-surface rounded-container shadow-lg p-3"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Close button */}
            <button
              className="absolute top-3 right-3 text-ink-muted hover:text-ink"
              onClick={closeModal}
              aria-label={t('close')}
            >
              <RiCloseLine size={16} />
            </button>

            {/* Header */}
            <div className="mb-3">
              <h2 className="text-title text-ink">{t('backgroundAgent.help.title')}</h2>
              <p className="mt-1 text-ink-muted">{t('backgroundAgent.help.subtitle')}</p>
            </div>

            {/* Content */}
            <div className="space-y-4">
              {/* Main Use Cases */}
              <div>
                <h3 className="text-heading mb-3 text-ink text-center">
                  {t('backgroundAgent.help.useCases.title')}
                </h3>
                <div className="space-y-2">
                  {/* Development Tasks */}
                  <div className="flex items-start p-2.5 bg-surface-2 border border-subtle rounded-container shadow-raised transition-shadow">
                    <div className="flex-shrink-0 mr-3">
                      <div className="w-8 h-8 bg-success-soft rounded-full flex items-center justify-center">
                        <FiCode className="w-4 h-4 text-success" />
                      </div>
                    </div>
                    <div className="flex-1">
                      <h4 className="text-heading mb-1 text-ink">
                        {t('backgroundAgent.help.useCases.development.title')}
                      </h4>
                      <p className="text-sm text-ink leading-relaxed">
                        {t('backgroundAgent.help.useCases.development.description')}
                      </p>
                    </div>
                  </div>

                  {/* Workflow Support */}
                  <div className="flex items-start p-2.5 bg-surface-2 border border-subtle rounded-container shadow-raised transition-shadow">
                    <div className="flex-shrink-0 mr-3">
                      <div className="w-8 h-8 bg-accent-tint rounded-full flex items-center justify-center">
                        <FiCpu className="w-4 h-4 text-accent" />
                      </div>
                    </div>
                    <div className="flex-1">
                      <h4 className="text-heading mb-1 text-ink">
                        {t('backgroundAgent.help.useCases.workflow.title')}
                      </h4>
                      <p className="text-sm text-ink leading-relaxed">
                        {t('backgroundAgent.help.useCases.workflow.description')}
                      </p>
                    </div>
                  </div>

                  {/* Business Automation */}
                  <div className="flex items-start p-2.5 bg-surface-2 border border-subtle rounded-container shadow-raised transition-shadow">
                    <div className="flex-shrink-0 mr-3">
                      <div className="w-8 h-8 bg-warning-soft rounded-full flex items-center justify-center">
                        <FiZap className="w-4 h-4 text-warning" />
                      </div>
                    </div>
                    <div className="flex-1">
                      <h4 className="text-heading mb-1 text-ink">
                        {t('backgroundAgent.help.useCases.business.title')}
                      </h4>
                      <p className="text-sm text-ink leading-relaxed">
                        {t('backgroundAgent.help.useCases.business.description')}
                      </p>
                    </div>
                  </div>
                </div>
              </div>

              {/* Prompt Tips */}
              <div className="bg-surface-2 p-2.5 rounded-container">
                <h4 className="text-subheading mb-1 text-ink flex items-center">
                  <FiCode className="w-4 h-4 mr-2 text-ink-muted" />
                  {t('backgroundAgent.help.prompts.title')}
                </h4>
                <p className="text-sm text-ink leading-relaxed">
                  {t('backgroundAgent.help.prompts.description')}
                </p>
              </div>
            </div>

            {/* Action Buttons */}
            <div className="flex justify-end mt-3 pt-2.5 border-t border-subtle">
              <Button variant="primary" onClick={closeModal}>
                {t('close')}
              </Button>
            </div>
          </div>
        </div>
      </div>
    ) : null
  }

  return {
    BackgroundAgentHelpModal,
    openModal,
    closeModal,
    isOpen
  }
}

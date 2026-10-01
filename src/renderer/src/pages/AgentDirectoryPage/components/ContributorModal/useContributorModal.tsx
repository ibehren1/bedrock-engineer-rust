import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { BsGithub } from 'react-icons/bs'
import { RiCloseLine } from 'react-icons/ri'
import { FiExternalLink } from 'react-icons/fi'
import { Button } from '@renderer/components/ui'

export const useContributorModal = () => {
  const [isOpen, setIsOpen] = useState(false)
  const { t } = useTranslation()

  const openModal = () => setIsOpen(true)
  const closeModal = () => setIsOpen(false)

  /**
   * Generates a GitHub issue URL with pre-filled information for contributing an agent
   */
  const getGitHubIssueUrl = () => {
    const issueTitle = encodeURIComponent('Add Custom Agent: [Your Agent Name]')
    const issueLabels = encodeURIComponent('enhancement,agent-directory')

    const bodyTemplate = `
## Agent Information

**Agent Name**: [Your Agent Name]

**Description**: [Brief description of what your agent does]

**Author**: [Your GitHub Username]

**Tags**: [Comma-separated list of relevant tags]

## YAML Content

\`\`\`yaml
# Please paste your exported YAML content here
name: "Your Agent Name"
description: "Brief description of what your agent does"
author: "your-github-username"
# Rest of your exported YAML...
\`\`\`

## Additional Notes

[Any additional information or context about your agent]
    `.trim()

    const issueBody = encodeURIComponent(bodyTemplate)

    return `https://github.com/aws-samples/bedrock-engineer/issues/new?title=${issueTitle}&labels=${issueLabels}&body=${issueBody}`
  }

  const ContributorModal = () => {
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
            className="relative w-full max-w-2xl bg-surface rounded-container shadow-lg p-3"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Close button */}
            <button
              className="absolute top-4 right-4 text-ink-muted hover:text-ink"
              onClick={closeModal}
              aria-label={t('close')}
            >
              <RiCloseLine size={24} />
            </button>

            {/* Header */}
            <div className="mb-3">
              <h2 className="text-title text-ink">{t('contributor.title')}</h2>
              <p className="mt-1 text-ink-muted">{t('contributor.subtitle')}</p>
            </div>

            {/* Content */}
            <div className="mb-3">
              <h3 className="text-heading font-medium mb-2 text-ink">{t('contributor.steps')}</h3>
              <ol className="list-decimal pl-5 space-y-3 text-ink">
                <li>
                  {t('contributor.step1')}
                  <div className="mt-1 p-3 bg-surface-2 rounded-control text-sm">
                    <code>{'[Your Agent Name] > ⋮ > Export > As Shared File'}</code>
                  </div>
                </li>
                <li>
                  {t('contributor.step2')}
                  <div className="mt-1 p-3 bg-surface-2 rounded-control text-sm">
                    <code
                      onClick={() => {
                        open(
                          'https://github.com/aws-samples/bedrock-engineer/tree/main/src/renderer/src/assets/directory-agents'
                        )
                      }}
                      className="hover:text-accent cursor-pointer"
                    >
                      src/renderer/src/assets/directory-agents/
                    </code>
                  </div>
                </li>
                <li>
                  {t('contributor.step3')}
                  <div className="mt-1 p-3 bg-surface-2 rounded-control text-sm">
                    <code className="whitespace-pre-wrap">{'author: "your-github-username"'}</code>
                  </div>
                </li>
                <li>{t('contributor.step4')}</li>
              </ol>
            </div>

            {/* Submit Options */}
            <div className="mt-3 pt-4 border-t border-subtle">
              <h3 className="text-heading font-medium mb-3 text-ink">
                {t('contributor.submitOptions')}
              </h3>

              <div className="flex flex-col md:flex-row gap-4">
                {/* GitHub Pull Request Option */}
                <div className="flex-1 p-2.5 border border-subtle rounded-container bg-surface-2">
                  <h4 className="font-medium mb-2 text-ink">{t('contributor.prOption')}</h4>
                  <p className="text-sm text-ink-muted mb-3">{t('contributor.prDescription')}</p>
                  <a
                    href="https://github.com/aws-samples/bedrock-engineer"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center text-sm text-accent hover:text-accent"
                  >
                    <BsGithub className="w-4 h-4 mr-1" />
                    <span>{t('contributor.viewRepo')}</span>
                    <FiExternalLink className="ml-1 w-3 h-3" />
                  </a>
                </div>

                {/* GitHub Issue Option */}
                <div className="flex-1 p-2.5 border border-subtle rounded-container bg-surface-2">
                  <h4 className="font-medium mb-2 text-ink">{t('contributor.issueOption')}</h4>
                  <p className="text-sm text-ink-muted mb-3">{t('contributor.issueDescription')}</p>
                  <a
                    href={getGitHubIssueUrl()}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="inline-flex items-center text-sm font-medium px-3 py-1.5 bg-accent hover:bg-accent-strong text-accent-fg rounded-control"
                  >
                    <BsGithub className="w-4 h-4 mr-1" />
                    <span>{t('contributor.createIssue')}</span>
                    <FiExternalLink className="ml-1 w-3 h-3" />
                  </a>
                </div>
              </div>
            </div>

            {/* Action Buttons */}
            <div className="flex justify-end mt-3">
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
    ContributorModal,
    openModal,
    closeModal,
    isOpen
  }
}

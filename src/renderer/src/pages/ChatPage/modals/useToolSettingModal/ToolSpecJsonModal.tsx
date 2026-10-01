import { Modal, Button } from 'flowbite-react'
import { memo } from 'react'
import { useTranslation } from 'react-i18next'
import JSONViewer from '@renderer/components/JSONViewer'
import type { Tool } from '@aws-sdk/client-bedrock-runtime'

interface ToolSpecJsonModalProps {
  isOpen: boolean
  onClose: () => void
  toolName: string
  toolSpec: Tool['toolSpec'] | undefined
}

const ToolSpecJsonModal = memo(
  ({ isOpen, onClose, toolName, toolSpec }: ToolSpecJsonModalProps) => {
    const { t } = useTranslation()

    return (
      <Modal
        dismissible
        size="4xl"
        show={isOpen}
        onClose={onClose}
        className="bg-surface border border-subtle shadow-lg"
      >
        <Modal.Header className="border-b border-subtle bg-surface text-ink rounded-t-container">
          <div className="flex items-center gap-2">
            <span>{toolName}</span>
            <span className="text-ink-muted">- Tool Specification</span>
          </div>
        </Modal.Header>

        <Modal.Body className="bg-surface rounded-b-container">
          <div className="w-full">
            {toolSpec ? (
              <JSONViewer
                data={toolSpec}
                title={t('Tool Specification (JSON)')}
                maxHeight="500px"
                showCopyButton={true}
              />
            ) : (
              <div className="text-center py-8 text-ink-muted">
                {t('No tool specification available')}
              </div>
            )}
          </div>
        </Modal.Body>

        <Modal.Footer className="bg-surface border-t border-subtle rounded-b-container">
          <Button onClick={onClose} className="bg-accent hover:bg-accent-strong text-ink">
            {t('Close')}
          </Button>
        </Modal.Footer>
      </Modal>
    )
  }
)

ToolSpecJsonModal.displayName = 'ToolSpecJsonModal'

export default ToolSpecJsonModal

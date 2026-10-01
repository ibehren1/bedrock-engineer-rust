import React, { useState } from 'react'
import MD from '@renderer/components/Markdown/MD'
import { Modal } from 'flowbite-react'

interface SystemPromptModalProps {
  isOpen: boolean
  onClose: () => void
  systemPrompt: string
}

export const useSystemPromptModal = () => {
  const [show, setShow] = useState(false)
  const handleOpen = () => {
    setShow(true)
  }
  const handleClose = () => {
    setShow(false)
  }

  return {
    show: show,
    handleOpen: handleOpen,
    handleClose: handleClose,
    SystemPromptModal: SystemPromptModal
  }
}

const SystemPromptModal = React.memo(
  ({ isOpen, onClose, systemPrompt }: SystemPromptModalProps) => {
    if (!isOpen) return null

    return (
      <Modal dismissible show={isOpen} onClose={onClose} size="7xl" className="bg-canvas">
        <div className="border-[0.5px] border-surface rounded-container shadow-xl">
          <Modal.Header className="border-b border-subtle bg-canvas rounded-t-container">
            SYSTEM PROMPT
          </Modal.Header>
          <Modal.Body className="p-0 bg-surface rounded-b-container">
            <div className="p-3 text-ink">
              <MD>{systemPrompt}</MD>
            </div>
          </Modal.Body>
        </div>
      </Modal>
    )
  }
)

SystemPromptModal.displayName = 'SystemPromptModal'

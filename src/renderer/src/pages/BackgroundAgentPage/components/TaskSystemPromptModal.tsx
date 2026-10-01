import React, { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Modal } from 'flowbite-react'
import MD from '@renderer/components/Markdown/MD'
import { ArrowPathIcon } from '@heroicons/react/24/outline'

interface TaskSystemPromptModalProps {
  isOpen: boolean
  onClose: () => void
  taskId: string | null
  taskName?: string
  getTaskSystemPrompt: (taskId: string) => Promise<string>
}

export const useTaskSystemPromptModal = () => {
  const [show, setShow] = useState(false)
  const [taskId, setTaskId] = useState<string | null>(null)
  const [taskName, setTaskName] = useState<string>('')

  const handleOpen = (id: string, name?: string) => {
    setTaskId(id)
    setTaskName(name || '')
    setShow(true)
  }

  const handleClose = () => {
    setShow(false)
    setTaskId(null)
    setTaskName('')
  }

  return {
    show,
    taskId,
    taskName,
    handleOpen,
    handleClose,
    TaskSystemPromptModal
  }
}

const TaskSystemPromptModal = React.memo(
  ({ isOpen, onClose, taskId, taskName, getTaskSystemPrompt }: TaskSystemPromptModalProps) => {
    const { t } = useTranslation()
    const [systemPrompt, setSystemPrompt] = useState<string>('')
    const [isLoading, setIsLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)

    // タスクIDが変更されたときにシステムプロンプトを取得
    useEffect(() => {
      const fetchSystemPrompt = async () => {
        if (!isOpen || !taskId) return

        try {
          setIsLoading(true)
          setError(null)
          const prompt = await getTaskSystemPrompt(taskId)
          setSystemPrompt(prompt)
        } catch (err: any) {
          console.error('Failed to fetch system prompt:', err)
          setError(err.message || t('backgroundAgent.errors.getSystemPrompt'))
        } finally {
          setIsLoading(false)
        }
      }

      fetchSystemPrompt()
    }, [isOpen, taskId, getTaskSystemPrompt, t])

    // モーダルが閉じられたときにステートをクリア
    useEffect(() => {
      if (!isOpen) {
        setSystemPrompt('')
        setError(null)
      }
    }, [isOpen])

    if (!isOpen) return null

    return (
      <Modal dismissible show={isOpen} onClose={onClose} size="7xl" className="bg-canvas">
        <div className="border-[0.5px] border-surface rounded-container shadow-xl">
          <Modal.Header className="border-b border-subtle bg-canvas rounded-t-container">
            <div className="flex items-center space-x-2">
              <span>{t('backgroundAgent.systemPrompt.title')}</span>
              {taskName && <span className="text-sm text-ink-muted">- {taskName}</span>}
            </div>
          </Modal.Header>
          <Modal.Body className="p-0 bg-surface rounded-b-container">
            <div className="p-3 text-ink">
              {isLoading ? (
                <div className="flex items-center justify-center py-8 h-[70vh]">
                  <ArrowPathIcon className="h-8 w-8 animate-spin text-accent" />
                  <span className="ml-2 text-ink-muted">
                    {t('backgroundAgent.systemPrompt.loading')}
                  </span>
                </div>
              ) : error ? (
                <div className="flex items-center justify-center py-8 h-[70vh]">
                  <div className="text-center">
                    <div className="text-danger mb-2">
                      {t('backgroundAgent.systemPrompt.error')}
                    </div>
                    <div className="text-sm text-ink-muted">{error}</div>
                  </div>
                </div>
              ) : systemPrompt ? (
                <div className="h-[70vh] overflow-y-auto">
                  <MD>{systemPrompt}</MD>
                </div>
              ) : (
                <div className="flex items-center justify-center py-8 h-[70vh]">
                  <div className="text-center text-ink-muted">
                    {t('backgroundAgent.systemPrompt.empty')}
                  </div>
                </div>
              )}
            </div>
          </Modal.Body>
        </div>
      </Modal>
    )
  }
)

TaskSystemPromptModal.displayName = 'TaskSystemPromptModal'

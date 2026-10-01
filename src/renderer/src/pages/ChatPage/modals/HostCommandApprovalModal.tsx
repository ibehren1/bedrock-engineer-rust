import React, { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Modal } from 'flowbite-react'
import { FiAlertTriangle } from 'react-icons/fi'
import {
  getPendingHostApproval,
  HostApprovalRequest,
  resolveHostApproval,
  subscribeHostApproval
} from '../lib/hostCommandApproval'

/**
 * Confirmation for a command the agent wants to run on the user's own machine rather
 * than in the chat's Docker sandbox. Mounted once by ChatPage; it shows itself whenever
 * the approval broker has a pending request.
 */
export const HostCommandApprovalModal: React.FC = () => {
  const { t } = useTranslation()
  const [request, setRequest] = useState<HostApprovalRequest | null>(getPendingHostApproval())

  useEffect(() => subscribeHostApproval(() => setRequest(getPendingHostApproval())), [])

  return (
    <Modal
      show={!!request}
      onClose={() => resolveHostApproval('deny')}
      size="lg"
      dismissible={false}
    >
      <Modal.Header>
        <div className="flex items-center gap-2">
          <FiAlertTriangle className="text-warning" />
          {t('hostCommand.title')}
        </div>
      </Modal.Header>
      <Modal.Body>
        <div className="space-y-2">
          <p className="text-sm text-ink">{t('hostCommand.body')}</p>

          <div className="rounded-control bg-canvas p-3 font-mono text-sm break-all">
            <div className="text-ink">{request?.command}</div>
            <div className="mt-2 text-xs text-ink-muted">
              {t('hostCommand.workingDirectory')}: {request?.cwd}
            </div>
          </div>

          <p className="text-xs text-ink-muted">{t('hostCommand.note')}</p>
        </div>
      </Modal.Body>
      <Modal.Footer>
        <div className="flex w-full justify-end gap-2">
          <button
            onClick={() => resolveHostApproval('deny')}
            className="px-2.5 py-1 text-sm rounded-control border border-strong text-ink hover:bg-raised"
          >
            {t('hostCommand.deny')}
          </button>
          <button
            onClick={() => resolveHostApproval('once')}
            className="px-2.5 py-1 text-sm rounded-control bg-accent text-accent-fg hover:bg-accent-strong"
          >
            {t('hostCommand.allowOnce')}
          </button>
          <button
            onClick={() => resolveHostApproval('chat')}
            className="px-2.5 py-1 text-sm rounded-control bg-accent-tint text-accent hover:bg-accent-tint-strong"
          >
            {t('hostCommand.allowForChat')}
          </button>
        </div>
      </Modal.Footer>
    </Modal>
  )
}

import { useCallback, useEffect, useState } from 'react'
import toast from 'react-hot-toast'
import { useTranslation } from 'react-i18next'

/** Mirrors `ChatAttachment` in the Rust `attachments` crate (src-tauri/crates/attachments). */
export interface ChatAttachment {
  name: string
  path: string
  size: number
  mtime: number
  kind: 'image' | 'text' | 'pdf' | 'docx' | 'other'
  extractable: boolean
}

interface AttachmentListing {
  directory: string
  files: ChatAttachment[]
}

interface AttachmentAddResult extends AttachmentListing {
  added: ChatAttachment[]
  errors: { name: string; error: string }[]
  canceled?: boolean
}

const EMPTY: AttachmentListing = { directory: '', files: [] }

/**
 * Track the files attached to the current chat.
 *
 * Unlike the sandbox this does not poll: the folder only changes through actions taken here,
 * and correctness never depends on the listing being current — the send path rebuilds the
 * context from disk. A window `focus` refresh covers files added or deleted in Finder.
 */
export const useChatAttachments = (sessionId?: string) => {
  const { t } = useTranslation()
  const [listing, setListing] = useState<AttachmentListing>(EMPTY)
  const [isBusy, setIsBusy] = useState(false)

  const refresh = useCallback(async () => {
    if (!sessionId) {
      setListing(EMPTY)
      return
    }
    try {
      setListing(await window.api.chatAttachments.list(sessionId))
    } catch {
      // A missing project directory and a missing folder look the same from here: no files.
      setListing(EMPTY)
    }
  }, [sessionId])

  useEffect(() => {
    void refresh()

    const onFocus = () => void refresh()
    window.addEventListener('focus', onFocus)
    return () => window.removeEventListener('focus', onFocus)
  }, [refresh])

  /** Report per-file failures with the message main produced, then show the new listing. */
  const applyResult = useCallback(
    (result: AttachmentAddResult) => {
      setListing({ directory: result.directory, files: result.files })

      for (const failure of result.errors) {
        toast.error(t('attachments.toast.addFailed', { name: failure.name, error: failure.error }))
      }
      if (result.added.length === 1) {
        toast.success(t('attachments.toast.added', { name: result.added[0].name }))
      } else if (result.added.length > 1) {
        toast.success(t('attachments.toast.addedMany', { count: result.added.length }))
      }
    },
    [t]
  )

  const addFiles = useCallback(
    async (files: File[]) => {
      if (files.length === 0) return
      if (!sessionId) {
        toast.error(t('attachments.toast.noSession'))
        return
      }

      setIsBusy(true)
      try {
        const payload = await Promise.all(
          files.map(async (file) => ({
            name: file.name,
            bytes: new Uint8Array(await file.arrayBuffer())
          }))
        )
        applyResult(await window.api.chatAttachments.add(sessionId, payload))
      } catch (error) {
        toast.error(
          t('attachments.toast.addFailed', {
            name: files.map((file) => file.name).join(', '),
            error: error instanceof Error ? error.message : String(error)
          })
        )
      } finally {
        setIsBusy(false)
      }
    },
    [sessionId, applyResult, t]
  )

  const addFromPicker = useCallback(async () => {
    if (!sessionId) {
      toast.error(t('attachments.toast.noSession'))
      return
    }

    setIsBusy(true)
    try {
      const result: AttachmentAddResult = await window.api.chatAttachments.addFromPicker(sessionId)
      if (!result.canceled) applyResult(result)
    } catch (error) {
      toast.error(
        t('attachments.toast.addFailed', {
          name: '',
          error: error instanceof Error ? error.message : String(error)
        })
      )
    } finally {
      setIsBusy(false)
    }
  }, [sessionId, applyResult, t])

  const remove = useCallback(
    async (name: string) => {
      if (!sessionId) return

      setIsBusy(true)
      try {
        const result = await window.api.chatAttachments.remove(sessionId, name)
        setListing({ directory: result.directory, files: result.files })
        if (result.removed) toast.success(t('attachments.toast.removed', { name }))
      } catch (error) {
        toast.error(
          t('attachments.toast.removeFailed', {
            name,
            error: error instanceof Error ? error.message : String(error)
          })
        )
      } finally {
        setIsBusy(false)
      }
    },
    [sessionId, t]
  )

  const openFolder = useCallback(async () => {
    if (!sessionId) return

    const result = await window.api.chatAttachments.openFolder(sessionId)
    if (!result.success) {
      toast.error(t('attachments.openFailed', { error: result.error ?? '' }))
    }
  }, [sessionId, t])

  return {
    files: listing.files,
    directory: listing.directory,
    totalSize: listing.files.reduce((sum, file) => sum + file.size, 0),
    isBusy,
    refresh,
    addFiles,
    addFromPicker,
    remove,
    openFolder
  }
}

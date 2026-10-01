import { useCallback } from 'react'
import toast from 'react-hot-toast'
import { useTranslation } from 'react-i18next'
import { nanoid } from 'nanoid'
import { CustomAgent } from '@/types/agent-chat'
import useSetting from '@renderer/hooks/useSetting'
import { PROTECTED_DEFAULT_AGENT_IDS } from '@renderer/pages/ChatPage/components/AgentList'

/**
 * Keep an imported agent distinguishable from one of the same name already in the list, so two
 * copies of "Software Developer" don't look like a duplicated row.
 */
function uniqueAgentName(name: string, existing: CustomAgent[]): string {
  const taken = new Set(existing.map((agent) => agent.name))
  if (!taken.has(name)) return name

  let suffix = 2
  while (taken.has(`${name} (${suffix})`)) suffix++
  return `${name} (${suffix})`
}

/**
 * Create / update / duplicate / delete (hide, for built-ins) and export
 * operations for the agents shown on the My Agents page.
 */
export const useAgentCrud = () => {
  const { customAgents, saveCustomAgents, loadSharedAgents, hideDefaultAgent } = useSetting()
  const { t } = useTranslation()

  const saveAgent = useCallback(
    (agent: CustomAgent, isExistingAgent: boolean): boolean => {
      if (!agent.name || !agent.name.trim()) {
        toast.error(t('Agent name is required'))
        return false
      }

      if (!agent.description || !agent.description.trim()) {
        toast.error(t('Agent description is required'))
        return false
      }

      try {
        const finalAgentData: CustomAgent = {
          ...agent,
          // mcpServersが未定義または空配列の場合は明示的に空配列を設定
          mcpServers: agent.mcpServers || []
        }

        const updatedAgents = isExistingAgent
          ? customAgents.map((a) => (a.id === agent.id ? finalAgentData : a))
          : [...customAgents, finalAgentData]

        saveCustomAgents(updatedAgents)
        toast.success(t('Agent saved successfully'))
        return true
      } catch (error) {
        console.error('Error saving agent:', error)
        toast.error(t('Failed to save agent'))
        return false
      }
    },
    [customAgents, saveCustomAgents, t]
  )

  const deleteAgent = useCallback(
    (id: string) => {
      // デフォルトエージェントは削除ではなく非表示にし、再シードされないように記録する
      const agent = customAgents.find((a) => a.id === id)
      const isDefaultAgent = agent ? agent.isCustom === false : false

      if (isDefaultAgent) {
        if (PROTECTED_DEFAULT_AGENT_IDS.includes(id)) return
        hideDefaultAgent(id)
        return
      }

      saveCustomAgents(customAgents.filter((a) => a.id !== id))
    },
    [customAgents, hideDefaultAgent, saveCustomAgents]
  )

  const duplicateAgent = useCallback(
    (agent: CustomAgent) => {
      const newAgent: CustomAgent = {
        ...agent,
        id: crypto.randomUUID(),
        name: `${agent.name} (${t('copy')})`,
        isCustom: true, // 明示的にtrueに設定して削除・編集可能にする
        // 共有プロパティを削除（複製されたエージェントは通常のカスタムエージェントとして扱う）
        isShared: undefined,
        organizationId: undefined
      }
      saveCustomAgents([...customAgents, newAgent])
    },
    [customAgents, saveCustomAgents, t]
  )

  const saveAsShared = useCallback(
    async (agent: CustomAgent) => {
      try {
        const result = await window.file.saveSharedAgent(agent)
        if (result.success) {
          // Load the updated shared agents to refresh the list in the UI
          await loadSharedAgents()
          toast.success(t('agentSavedAsShared'), { duration: 5000 })
        } else {
          console.error('Failed to save agent as shared file:', result.error)
          toast.error(result.error || t('failedToSaveShared'))
        }
      } catch (error) {
        console.error('Error saving shared agent:', error)
        toast.error(t('failedToSaveShared'))
      }
    },
    [loadSharedAgents, t]
  )

  const deleteSharedFile = useCallback(
    async (agent: CustomAgent) => {
      if (!agent.sharedFilePath) return

      try {
        const result = await window.file.deleteSharedAgent(agent.sharedFilePath)
        if (result.canceled) return

        if (result.success) {
          // The file is gone; re-read the directory so the shared entry leaves the list
          await loadSharedAgents()
          toast.success(t('sharedFileDeleted'), { duration: 5000 })
        } else {
          console.error('Failed to delete shared agent file:', result.error)
          toast.error(result.error || t('failedToDeleteSharedFile'))
        }
      } catch (error) {
        console.error('Error deleting shared agent file:', error)
        toast.error(t('failedToDeleteSharedFile'))
      }
    },
    [loadSharedAgents, t]
  )

  const downloadYaml = useCallback(
    async (agent: CustomAgent) => {
      try {
        const result = await window.file.exportAgentYaml(agent)
        if (result.canceled) return

        if (result.success) {
          toast.success(t('agentYamlDownloaded', { path: result.filePath }), { duration: 5000 })
        } else {
          console.error('Failed to download agent YAML:', result.error)
          toast.error(result.error || t('failedToDownloadYaml'))
        }
      } catch (error) {
        console.error('Error downloading agent YAML:', error)
        toast.error(t('failedToDownloadYaml'))
      }
    },
    [t]
  )

  const importAgent = useCallback(async () => {
    try {
      const result = await window.file.importAgentFile()
      if (result.canceled) return

      if (!result.success || !result.agent) {
        console.error('Failed to import agent:', result.error)
        toast.error(result.error || t('failedToImportAgent'))
        return
      }

      // An imported agent becomes the user's own: a fresh id, editable, and none of the sharing
      // flags the source file may have carried.
      const imported: CustomAgent = {
        ...result.agent,
        id: `custom_agent_${nanoid(8)}`,
        name: uniqueAgentName(result.agent.name, customAgents),
        isCustom: true,
        isShared: undefined,
        directoryOnly: undefined,
        organizationId: undefined,
        sharedFilePath: undefined,
        mcpTools: undefined,
        mcpServers: result.agent.mcpServers || []
      }

      saveCustomAgents([...customAgents, imported])
      toast.success(t('agentImported', { name: imported.name }), { duration: 5000 })
    } catch (error) {
      console.error('Error importing agent:', error)
      toast.error(t('failedToImportAgent'))
    }
  }, [customAgents, saveCustomAgents, t])

  const convertToStrands = useCallback(async (agentId: string) => {
    try {
      const directory = await window.api.openDirectory()
      if (!directory) {
        return // User cancelled directory selection
      }

      const loadingToast = toast.loading('Converting agent to Strands Agents...')
      const result = await window.api.strandsConverter.convertAndSave(agentId, directory)
      toast.dismiss(loadingToast)

      if (result.success) {
        toast.success(`Strands Agents files saved to ${directory}`, { duration: 5000 })
      } else {
        toast.error(result.error || 'Failed to convert agent to Strands Agents')
        console.error('Conversion failed:', result.errors)
      }
    } catch (error) {
      console.error('Error converting agent to Strands Agents:', error)
      toast.error('Failed to convert agent to Strands Agents')
    }
  }, [])

  return {
    saveAgent,
    deleteAgent,
    duplicateAgent,
    saveAsShared,
    deleteSharedFile,
    downloadYaml,
    importAgent,
    convertToStrands
  }
}

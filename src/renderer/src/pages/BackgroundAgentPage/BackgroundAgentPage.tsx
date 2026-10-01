import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { BsQuestionCircle } from 'react-icons/bs'
import { useBackgroundAgent } from './hooks/useBackgroundAgent'
import { TaskFormModal } from './components/TaskFormModal'
import { TaskList } from './components/TaskList'
import { useBackgroundAgentHelpModal } from './components/BackgroundAgentHelpModal'

const BackgroundAgentPage: React.FC = () => {
  const { t } = useTranslation()
  const [showCreateForm, setShowCreateForm] = useState(false)

  const {
    tasks,
    isLoading,
    taskLoadingStates,
    createTask,
    updateTask,
    cancelTask,
    toggleTask,
    executeTaskManually,
    getTaskSystemPrompt,
    refreshAll
  } = useBackgroundAgent()

  // Use the background agent help modal hook
  const { BackgroundAgentHelpModal, openModal } = useBackgroundAgentHelpModal()

  const handleCreateTask = async (taskConfig: any) => {
    try {
      await createTask(taskConfig)
      setShowCreateForm(false)
      await refreshAll()
    } catch (error) {
      console.error('Failed to create task:', error)
    }
  }

  return (
    <div className="px-2.5 py-1.5">
      <header className="mb-3">
        <h1 className="text-title mb-2 text-ink">Background Agent</h1>
        <div className="flex items-center">
          <p className="text-ink-muted">{t('backgroundAgent.pageDescription')}</p>
          <div className="relative ml-2 group">
            <BsQuestionCircle
              className="w-4 h-4 text-ink-muted hover:text-accent cursor-pointer"
              onClick={openModal}
            />
            <div
              className="absolute bottom-full left-1/2 transform -translate-x-1/2 mb-2 px-3 py-1 text-xs
                            font-medium bg-ink text-canvas rounded-container shadow-sm opacity-0 group-hover:opacity-100
                            transition-opacity duration-300 whitespace-nowrap pointer-events-none"
            >
              {t('backgroundAgent.help.tooltip')}
              <div className="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-ink"></div>
            </div>
          </div>
        </div>
      </header>

      <TaskList
        tasks={tasks}
        isLoading={isLoading}
        taskLoadingStates={taskLoadingStates}
        onToggleTask={toggleTask}
        onCancelTask={cancelTask}
        onExecuteTask={executeTaskManually}
        onUpdateTask={updateTask}
        onRefresh={refreshAll}
        onGetTaskSystemPrompt={getTaskSystemPrompt}
        onCreateTask={() => setShowCreateForm(true)}
      />

      {/* Create Task Modal */}
      {showCreateForm && (
        <TaskFormModal
          mode="create"
          onSubmit={handleCreateTask}
          onCancel={() => setShowCreateForm(false)}
        />
      )}

      {/* Background Agent Help Modal */}
      <BackgroundAgentHelpModal />
    </div>
  )
}

export default BackgroundAgentPage

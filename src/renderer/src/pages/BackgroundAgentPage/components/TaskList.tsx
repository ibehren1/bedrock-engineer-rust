import React, { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ArrowPathIcon, CalendarIcon, PlusIcon } from '@heroicons/react/24/outline'
import { ScheduledTask, ScheduleConfig } from '../hooks/useBackgroundAgent'
import { TaskViewToggle } from './TaskList/TaskViewToggle'
import { TaskListView } from './TaskList/TaskListView'
import { TaskTableView } from './TaskList/TaskTableView'

interface TaskListProps {
  tasks: ScheduledTask[]
  isLoading: boolean
  taskLoadingStates: { [taskId: string]: boolean }
  onToggleTask: (taskId: string, enabled: boolean) => Promise<void>
  onCancelTask: (taskId: string) => Promise<void>
  onExecuteTask: (taskId: string) => Promise<void>
  onUpdateTask: (taskId: string, config: ScheduleConfig) => Promise<void>
  onRefresh: () => Promise<void>
  onGetTaskSystemPrompt: (taskId: string) => Promise<string>
  onCreateTask: () => void
}

export const TaskList: React.FC<TaskListProps> = ({
  tasks,
  isLoading,
  taskLoadingStates,
  onToggleTask,
  onCancelTask,
  onExecuteTask,
  onUpdateTask,
  onRefresh,
  onGetTaskSystemPrompt,
  onCreateTask
}) => {
  const { t } = useTranslation()
  const [isTableView, setIsTableView] = useState(false)

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <ArrowPathIcon className="h-8 w-8 animate-spin text-accent" />
      </div>
    )
  }

  if (tasks.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-center">
        <CalendarIcon className="h-12 w-12 text-ink-faint mb-4" />
        <h3 className="text-heading font-medium text-ink mb-2">{t('backgroundAgent.noTasks')}</h3>
        <p className="text-ink-muted mb-4">{t('backgroundAgent.noTasksDescription')}</p>
        <button
          onClick={onCreateTask}
          className="inline-flex items-center px-2.5 py-1 border border-transparent text-sm font-medium rounded-control text-accent-fg bg-accent hover:bg-accent-strong focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent"
        >
          <PlusIcon className="h-4 w-4 mr-2" />
          {t('backgroundAgent.createTask')}
        </button>
      </div>
    )
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center space-x-4">
          <h2 className="text-heading font-medium text-ink">
            {t('backgroundAgent.scheduledTasks')} ({tasks.length})
          </h2>
          <TaskViewToggle isTableView={isTableView} onToggle={setIsTableView} />
        </div>
        <div className="flex items-center space-x-3">
          <button
            onClick={onCreateTask}
            className="inline-flex items-center px-3 py-2 border border-transparent text-sm leading-4 font-medium rounded-control shadow-sm text-accent-fg bg-accent hover:bg-accent-strong focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent transition-colors duration-200"
          >
            <PlusIcon className="h-4 w-4 mr-2" />
            {t('backgroundAgent.createTask')}
          </button>
          <button
            onClick={onRefresh}
            className="inline-flex items-center px-3 py-2 border border-strong shadow-sm text-sm leading-4 font-medium rounded-control text-ink bg-surface hover:bg-surface-2 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent transition-colors duration-200"
          >
            <ArrowPathIcon className="h-4 w-4 mr-2" />
            {t('common.refresh')}
          </button>
        </div>
      </div>

      {/* View Content */}
      {isTableView ? (
        <TaskTableView
          tasks={tasks}
          taskLoadingStates={taskLoadingStates}
          onToggleTask={onToggleTask}
          onCancelTask={onCancelTask}
          onExecuteTask={onExecuteTask}
          onUpdateTask={onUpdateTask}
          onGetTaskSystemPrompt={onGetTaskSystemPrompt}
        />
      ) : (
        <TaskListView
          tasks={tasks}
          taskLoadingStates={taskLoadingStates}
          onToggleTask={onToggleTask}
          onCancelTask={onCancelTask}
          onExecuteTask={onExecuteTask}
          onUpdateTask={onUpdateTask}
          onGetTaskSystemPrompt={onGetTaskSystemPrompt}
        />
      )}
    </div>
  )
}

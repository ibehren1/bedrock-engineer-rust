import { IconType } from 'react-icons'
import { FiHome, FiFeather, FiSettings } from 'react-icons/fi'
import { LuCombine, LuBookDown } from 'react-icons/lu'
import { HiOutlineChatAlt2 } from 'react-icons/hi'
import { BsLayoutWtf } from 'react-icons/bs'
import { PiPulse } from 'react-icons/pi'
import { TbRobot } from 'react-icons/tb'
import HomePage from './pages/HomePage/HomePage'
import SettingPage from './pages/SettingPage/SettingPage'
import StepFunctionsGeneratorPage from './pages/StepFunctionsGeneratorPage/StepFunctionsGeneratorPage'
import WebsiteGeneratorPage from './pages/WebsiteGeneratorPage/WebsiteGeneratorPage'
import ChatPage from './pages/ChatPage/ChatPage'
import DiagramGeneratorPage from './pages/DiagramGeneratorPage/DiagramGeneratorPage'
import { AgentDirectoryPage } from './pages/AgentDirectoryPage/AgentDirectoryPage'
import { MyAgentsPage } from './pages/MyAgentsPage/MyAgentsPage'
import BackgroundAgentPage from './pages/BackgroundAgentPage/BackgroundAgentPage'
import TaskExecutionHistoryPage from './pages/BackgroundAgentPage/TaskExecutionHistoryPage'

export interface AppRoute {
  name: string
  href: string
  icon: IconType
  position: 'top' | 'hidden'
  element: React.ReactElement
}

export const routes: AppRoute[] = [
  {
    name: 'Home',
    href: '/',
    icon: FiHome,
    position: 'top',
    element: <HomePage />
  },
  {
    name: 'Chat',
    href: '/chat',
    icon: HiOutlineChatAlt2,
    position: 'top',
    element: <ChatPage />
  },
  {
    name: 'My Agents',
    href: '/my-agents',
    icon: TbRobot,
    position: 'top',
    element: <MyAgentsPage />
  },
  {
    name: 'Agent Directory',
    href: '/agent-directory',
    icon: LuBookDown,
    position: 'top',
    element: <AgentDirectoryPage />
  },
  {
    name: 'Background Agent',
    href: '/background-agent',
    icon: PiPulse,
    position: 'top',
    element: <BackgroundAgentPage />
  },
  {
    name: 'Website Generator',
    href: '/generative-ui',
    icon: FiFeather,
    position: 'top',
    element: <WebsiteGeneratorPage />
  },
  {
    name: 'Step Functions Generator',
    href: '/step-functions-generator',
    icon: LuCombine,
    position: 'top',
    element: <StepFunctionsGeneratorPage />
  },
  {
    name: 'Diagram Generator',
    href: '/diagram-generator',
    icon: BsLayoutWtf,
    position: 'top',
    element: <DiagramGeneratorPage />
  },
  {
    name: 'Settings',
    href: '/setting',
    icon: FiSettings,
    position: 'top',
    element: <SettingPage />
  },
  {
    name: 'Task History',
    href: '/background-agent/task-history/:taskId',
    icon: PiPulse,
    position: 'hidden', // Hidden from navigation menu
    element: <TaskExecutionHistoryPage />
  }
  // for debug
  // {
  //   name: 'Error',
  //   href: '/error',
  //   icon: FiSettings,
  //   position: 'top',
  //   element: <ErrorPage />
  // }
]

/**
 * Routes that belong to a page in `routes` rather than to the navigation.
 * Kept separate because `routes` drives the sidebar and the command palette,
 * and a parameterized path has no business appearing in either.
 */
export const subRoutes: { href: string; element: React.ReactElement }[] = [
  { href: '/setting/:tab', element: <SettingPage /> }
]

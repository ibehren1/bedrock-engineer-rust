import React from 'react'
import { FiGithub, FiHelpCircle } from 'react-icons/fi'
import { Flowbite, Tooltip } from 'flowbite-react'
import { flowbiteTheme } from './flowbiteTheme'
import { createHashRouter, Link, Outlet, RouterProvider, useLocation } from 'react-router-dom'
import CmdK from './command-palette'
import { routes, subRoutes } from './routes'
import { isRouteActive } from './lib/routeMatching'
import HomePage from './pages/HomePage/HomePage'
import { Toaster } from 'react-hot-toast'
import ErrorPage from './pages/ErrorPage/ErrorPage'
import { SettingsProvider } from './contexts/SettingsContext'
import useSetting from './hooks/useSetting'
import { ChatHistoryProvider } from './contexts/ChatHistoryContext'
import { AgentDirectoryProvider } from './contexts/AgentDirectoryContext'
import { StepType, TourProvider } from '@reactour/tour'
import { useTranslation } from 'react-i18next'
import { ToastService } from './services/ToastService'
import { useTheme } from './hooks/useTheme'

const ListItem: React.FC<{
  children: any
  selected?: boolean
  toolTipContent?: string
  href
}> = ({ children, selected, toolTipContent, href }) => {
  // Active state is a solid accent, not the old blue-to-violet gradient fill.
  // text-accent-fg rather than text-white: in charcoal the accent is amber and
  // needs dark text drawn on it.
  const bgColor = selected
    ? 'bg-accent text-accent-fg'
    : 'text-ink-muted hover:bg-raised hover:text-ink'
  // p-1.5/m-0.5 rather than p-3/m-1: at 16px icons this gives a 28px hit target
  // in a 40px rail, instead of 44px in a 56px one.
  const base =
    'p-1.5 cursor-pointer m-0.5 rounded-control transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent '
  return (
    <Link to={href} className={href === '/setting' ? 'react-tour-first-step' : ''}>
      {toolTipContent ? (
        <Tooltip content={toolTipContent} placement="right" animation="duration-500">
          <li className={base + bgColor}>{children}</li>
        </Tooltip>
      ) : (
        <li className={base + bgColor}>{children}</li>
      )}
    </Link>
  )
}

const Layout: React.FC = () => {
  const location = useLocation()
  const { t } = useTranslation()
  const { sidebarHiddenItems } = useSetting()
  // Keep <html data-theme> in sync with the appearance setting (and OS changes
  // when 'system' is selected).
  useTheme()

  // タスク履歴ウィンドウかどうかを判定
  const isTaskHistoryWindow = location.pathname.includes('/task-history/')

  // タスク履歴ウィンドウの場合はシンプルなレイアウト
  if (isTaskHistoryWindow) {
    return (
      <div className="bg-canvas min-h-screen h-screen">
        <Outlet />
      </div>
    )
  }

  // 通常のレイアウト（サイドバー付き）
  return (
    <div className="bg-canvas">
      <div className="flex min-h-screen h-screen">
        <div className="bg-surface m-1 border border-subtle rounded-control">
          <nav className="flex flex-col justify-between h-full">
            <ul>
              {routes
                .map((page, index) => ({ page, shortcut: index + 1 }))
                .filter(
                  ({ page }) =>
                    page.position !== 'hidden' && !sidebarHiddenItems.includes(page.href)
                )
                .map(({ page, shortcut }) => {
                  return (
                    <ListItem
                      key={page.name}
                      selected={isRouteActive(location.pathname, page.href)}
                      href={page.href}
                      toolTipContent={page.name + ' ⌘ ' + shortcut}
                    >
                      <page.icon className="w-4 h-4" />
                    </ListItem>
                  )
                })}
            </ul>
            <ul>
              {/* Opens a chat with the bundled user guide attached. Not a route: keeping it out
                  of `routes` leaves the ⌘1..⌘N numbering and the sidebar settings list alone. */}
              <ListItem href="/chat?help=1" toolTipContent={t('help.sidebarTooltip')}>
                <FiHelpCircle className="w-4 h-4" />
              </ListItem>
              <div onClick={() => open('https://github.com/ibehren1/bedrock-engineer-rust')}>
                <ListItem href="#">
                  <FiGithub className="w-4 h-4" />
                </ListItem>
              </div>
            </ul>
          </nav>
        </div>
        <CmdK />

        <div className="flex-1 bg-canvas overflow-x-auto">
          <Outlet />
        </div>
      </div>
    </div>
  )
}

const router = createHashRouter([
  {
    path: '/',
    element: <Layout />,
    errorElement: <ErrorPage />,
    children: [
      ...routes.map((route) => ({
        path: route.href === '/' ? '/' : route.href,
        element: route.element,
        index: route.href === '/'
      })),
      ...subRoutes.map((route) => ({
        path: route.href,
        element: route.element
      })),
      {
        path: '*',
        element: <HomePage />
      }
    ]
  }
])

const styles: any = {
  maskWrapper: (base) => ({
    ...base
  }),
  maskArea: (base) => ({
    ...base,
    rx: 5
  }),
  popover: (base) => ({
    ...base,
    // Was a hardcoded orange (#ef5a3d), the one accent in the app that did not
    // come from the theme.
    '--reactour-accent': 'var(--accent)',
    backgroundColor: 'var(--surface)',
    color: 'var(--ink)',
    borderRadius: 5
  }),
  badge: (base) => ({
    ...base,
    color: 'var(--accent-fg)',
    backgroundColor: 'var(--accent)'
  })
}

function App(): JSX.Element {
  const { t } = useTranslation()

  const steps: StepType[] = [
    {
      selector: '.react-tour-first-step',
      content: t('set your aws credential'),
      position: 'right'
    }
  ]

  return (
    // Flowbite components rendered with stock defaults until now — there was no
    // provider anywhere, so all 42 import sites used marketing-site spacing.
    // See flowbiteTheme.ts. `mode: 'light'` keeps Flowbite's own dark-mode
    // toggle out of the way: this app drives appearance from data-theme, and
    // letting Flowbite manage a `dark` class as well would give two competing
    // sources of truth.
    <Flowbite theme={{ theme: flowbiteTheme, mode: 'light' }}>
      <TourProvider steps={steps} styles={styles}>
        <SettingsProvider>
          <ChatHistoryProvider>
            <AgentDirectoryProvider>
              <div>
                <Toaster {...ToastService.getToasterConfig()} />
                <RouterProvider router={router} />
              </div>
            </AgentDirectoryProvider>
          </ChatHistoryProvider>
        </SettingsProvider>
      </TourProvider>
    </Flowbite>
  )
}

export default App

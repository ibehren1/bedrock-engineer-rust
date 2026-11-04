import React, { useEffect } from 'react'
import { modKeyLabel, pageShortcut } from './lib/platform'
import { FiGithub, FiHelpCircle } from 'react-icons/fi'
import { ThemeProvider, Tooltip } from 'flowbite-react'
import { ThemeInit } from '../../../.flowbite-react/init'
import { flowbiteClearTheme, flowbiteTheme } from './flowbiteTheme'
import {
  createHashRouter,
  Link,
  Outlet,
  RouterProvider,
  useLocation,
  useNavigate
} from 'react-router'
import CmdK from './command-palette'
import { routes, subRoutes } from './routes'
import { isRouteActive, isStandaloneWindowRoute } from './lib/routeMatching'
import HomePage from './pages/HomePage/HomePage'
import { Toaster } from 'react-hot-toast'
import ErrorPage from './pages/ErrorPage/ErrorPage'
import { SettingsProvider } from './contexts/SettingsContext'
import useSetting from './hooks/useSetting'
import { ChatHistoryProvider } from './contexts/ChatHistoryContext'
import { AgentDirectoryProvider } from './contexts/AgentDirectoryContext'
import { MemoryProvider } from './contexts/MemoryContext'
import { AppUpdateProvider } from './contexts/AppUpdateContext'
import { UpdateBanner } from './components/UpdateBanner'
import { TourProvider, useTour } from '@reactour/tour'
import { buildTourSteps } from './tourSteps'
import { useTranslation } from 'react-i18next'
import { ToastService } from './services/ToastService'
import { useTheme } from './hooks/useTheme'
import { useBudgetStatus } from './pages/AccountingPage/useBudgetStatus'

/** The command palette shortcut as shown to the user: ⌘K on macOS, Ctrl+K elsewhere. */
const paletteShortcutLabel = (): string => (modKeyLabel() === '⌘' ? '⌘K' : `${modKeyLabel()}+K`)

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
    'p-1.5 cursor-pointer m-0.5 rounded-control transition-colors focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-accent '
  return (
    // data-tour: the app tour's stops point at sidebar items by href (tourSteps.ts).
    <Link to={href} data-tour={href}>
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
  const navigate = useNavigate()
  const { sidebarHiddenItems } = useSetting()
  // Badge on the Accounting icon while this month is over budget.
  const budget = useBudgetStatus()
  // Keep <html data-theme> in sync with the appearance setting (and OS changes
  // when 'system' is selected).
  useTheme()

  // The tour skips sidebar items hidden in Settings → Sidebar, and follows the language.
  const { setSteps } = useTour()
  useEffect(() => {
    setSteps?.(buildTourSteps(t, sidebarHiddenItems, paletteShortcutLabel()))
  }, [setSteps, t, sidebarHiddenItems])

  // App menu > Settings… (Rust only sends it to the main window).
  useEffect(() => window.appWindow.onOpenSettings(() => navigate('/setting')), [navigate])

  // タスク履歴ウィンドウ・ファイルビューアウィンドウかどうかを判定
  const isStandaloneWindow = isStandaloneWindowRoute(location.pathname)

  // 別ウィンドウの場合はシンプルなレイアウト（サイドバーなし）
  if (isStandaloneWindow) {
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
                      toolTipContent={
                        pageShortcut(shortcut)
                          ? `${page.name} ${modKeyLabel()}${modKeyLabel() === '⌘' ? ' ' : '+'}${shortcut}`
                          : page.name
                      }
                    >
                      {page.href === '/accounting' && budget?.overBudget ? (
                        <span className="relative block">
                          <page.icon className="w-4 h-4" />
                          <span
                            className="absolute -top-0.5 -right-0.5 w-1.5 h-1.5 rounded-full bg-danger"
                            aria-label={t('accounting.budget.overBadge')}
                          />
                        </span>
                      ) : (
                        <page.icon className="w-4 h-4" />
                      )}
                    </ListItem>
                  )
                })}
            </ul>
            <ul>
              {/* Opens the user guide (works offline); its "Ask a question" starts the Help chat.
                  A hidden route, so the ⌘1..⌘9 (Ctrl+1..9) numbering and the sidebar settings list
                  are unaffected. */}
              <ListItem
                href="/help"
                selected={isRouteActive(location.pathname, '/help')}
                toolTipContent={t('help.sidebarTooltip')}
              >
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
        <UpdateBanner />
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
    // On the popover's top edge rather than hanging off its left corner: the last stop (Help,
    // at the bottom of the sidebar) puts the popover flush with the window's left edge, which
    // clipped the step number.
    left: '0.75em',
    color: 'var(--accent-fg)',
    backgroundColor: 'var(--accent)'
  })
}

function App(): React.JSX.Element {
  const { t } = useTranslation()

  // Every stop; Layout narrows this to the items shown in the sidebar.
  const steps = buildTourSteps(t, [], paletteShortcutLabel())

  return (
    // Flowbite components rendered with stock defaults until now — there was no
    // provider anywhere, so all 42 import sites used marketing-site spacing.
    // See flowbiteTheme.ts. Flowbite's own dark-mode switching (useThemeMode /
    // ThemeModeScript) is never used: this app drives appearance from data-theme,
    // and letting Flowbite manage a `dark` class as well would give two competing
    // sources of truth. ThemeInit tells Flowbite it runs on Tailwind CSS 4
    // (.flowbite-react/config.json), so it converts its stock classes.
    <ThemeProvider theme={flowbiteTheme} clearTheme={flowbiteClearTheme}>
      <ThemeInit />
      <TourProvider steps={steps} styles={styles}>
        <SettingsProvider>
          <ChatHistoryProvider>
            <AgentDirectoryProvider>
              <MemoryProvider>
                <AppUpdateProvider>
                  <div>
                    <Toaster {...ToastService.getToasterConfig()} />
                    <RouterProvider router={router} />
                  </div>
                </AppUpdateProvider>
              </MemoryProvider>
            </AgentDirectoryProvider>
          </ChatHistoryProvider>
        </SettingsProvider>
      </TourProvider>
    </ThemeProvider>
  )
}

export default App

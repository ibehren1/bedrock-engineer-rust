import { useTranslation } from 'react-i18next'
import { SettingSection } from '../SettingSection'
import useSetting from '@renderer/hooks/useSetting'
import { routes } from '@renderer/routes'

// Settings is always visible so users can re-enable hidden items.
const ALWAYS_VISIBLE_HREFS = ['/setting']

export const SidebarSection = () => {
  const { t } = useTranslation()
  const { sidebarHiddenItems, setSidebarHiddenItems } = useSetting()

  const hideableRoutes = routes.filter(
    (route) => route.position === 'top' && !ALWAYS_VISIBLE_HREFS.includes(route.href)
  )

  const toggleItem = (href: string, show: boolean) => {
    if (show) {
      setSidebarHiddenItems(sidebarHiddenItems.filter((item) => item !== href))
    } else if (!sidebarHiddenItems.includes(href)) {
      setSidebarHiddenItems([...sidebarHiddenItems, href])
    }
  }

  return (
    <SettingSection title={t('sidebar.title')} description={t('sidebar.description')}>
      <div className="flex flex-col space-y-2">
        {hideableRoutes.map((route) => {
          const visible = !sidebarHiddenItems.includes(route.href)
          const inputId = `sidebar-item-${route.href}`
          return (
            <div key={route.href} className="flex items-center space-x-2">
              <input
                type="checkbox"
                id={inputId}
                checked={visible}
                onChange={(e) => toggleItem(route.href, e.target.checked)}
                className="h-4 w-4 rounded-control border-strong text-accent focus:ring-accent"
              />
              <label htmlFor={inputId} className="text-sm text-ink flex items-center gap-2">
                <route.icon className="text-base" />
                {t(`sidebar.items.${route.name}`, route.name)}
              </label>
            </div>
          )
        })}
      </div>
    </SettingSection>
  )
}

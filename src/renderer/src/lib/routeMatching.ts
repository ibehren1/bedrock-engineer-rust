/**
 * Whether a nav entry should read as active for the current location. A page's
 * own sub-routes (`/setting/aws`, `/background-agent/task-history/…`) keep their
 * parent highlighted; `'/'` only ever matches itself.
 *
 * Kept out of `routes.tsx` so it can be unit-tested without importing every page.
 */
export const isRouteActive = (pathname: string, href: string): boolean => {
  if (pathname === href) return true
  if (href === '/') return false
  return pathname.startsWith(`${href}/`)
}

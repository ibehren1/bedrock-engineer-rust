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

/** The pop-out file viewer's route (`?path=<root-relative>`); matches `file_viewer_route` in Rust. */
export const FILE_VIEWER_ROUTE = '/file-viewer'

/** Routes shown in a window of their own, without the app sidebar. */
export const isStandaloneWindowRoute = (pathname: string): boolean =>
  pathname.startsWith('/background-agent/task-history/') || pathname === FILE_VIEWER_ROUTE

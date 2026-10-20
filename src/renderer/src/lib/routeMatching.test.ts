import { isRouteActive, isStandaloneWindowRoute } from './routeMatching'

describe('isRouteActive', () => {
  it('matches the route itself', () => {
    expect(isRouteActive('/setting', '/setting')).toBe(true)
    expect(isRouteActive('/', '/')).toBe(true)
  })

  it('keeps a parent nav entry active on its sub-routes', () => {
    expect(isRouteActive('/setting/aws', '/setting')).toBe(true)
    expect(isRouteActive('/setting/models', '/setting')).toBe(true)
    expect(isRouteActive('/background-agent/task-history/abc', '/background-agent')).toBe(true)
  })

  it('never treats Home as a prefix of everything', () => {
    expect(isRouteActive('/setting', '/')).toBe(false)
    expect(isRouteActive('/chat', '/')).toBe(false)
  })

  it('does not match unrelated routes that share a prefix string', () => {
    expect(isRouteActive('/settings-other', '/setting')).toBe(false)
    expect(isRouteActive('/chat', '/setting')).toBe(false)
  })
})

describe('isStandaloneWindowRoute', () => {
  it('matches the task history and file viewer windows only', () => {
    expect(isStandaloneWindowRoute('/background-agent/task-history/t1')).toBe(true)
    expect(isStandaloneWindowRoute('/file-viewer')).toBe(true)
    expect(isStandaloneWindowRoute('/background-agent')).toBe(false)
    expect(isStandaloneWindowRoute('/chat')).toBe(false)
    expect(isStandaloneWindowRoute('/file-viewer-other')).toBe(false)
  })
})

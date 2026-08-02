import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { Route, formatRouteHash, parseRouteHash } from './route.svelte'

describe('issue route', () => {
  beforeEach(() => {
    window.history.replaceState(null, '', '/')
  })

  afterEach(() => {
    window.history.replaceState(null, '', '/')
  })

  it.each([
    ['', { space: null, issue: null }],
    ['#s=o-r', { space: 'o-r', issue: null }],
    ['#s=o-r&i=12', { space: 'o-r', issue: 12 }],
    ['#s=a%20b&i=0', { space: 'a b', issue: null }],
  ])('parses %s without losing valid space or accepting a non-positive issue', (hash, want) => {
    expect(parseRouteHash(hash)).toEqual(want)
  })

  it.each(['-2', '1.5', 'abc', '9007199254740992'])(
    'rejects unsafe issue value %s instead of routing to it',
    (issue) => {
      expect(parseRouteHash(`#s=o-r&i=${issue}`)).toEqual({ space: 'o-r', issue: null })
    },
  )

  it('emits encoded space and issue parameters in canonical order', () => {
    expect(formatRouteHash('a b', 12)).toBe('#s=a+b&i=12')
  })

  it('tracks hash changes until its owned listener is destroyed', () => {
    const route = new Route(window)

    route.go('o-r', 12)
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    expect({ space: route.space, issue: route.issue }).toEqual({ space: 'o-r', issue: 12 })

    route.destroy()
    window.location.hash = '#s=other&i=99'
    window.dispatchEvent(new HashChangeEvent('hashchange'))

    expect({ space: route.space, issue: route.issue }).toEqual({ space: 'o-r', issue: 12 })
  })

  it('updates reactive state immediately for programmatic navigation', () => {
    const route = new Route(window)

    route.go('second', 22)

    expect(window.location.hash).toBe('#s=second&i=22')
    expect({ space: route.space, issue: route.issue }).toEqual({ space: 'second', issue: 22 })
    route.destroy()
  })
})

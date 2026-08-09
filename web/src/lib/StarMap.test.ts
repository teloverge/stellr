/// <reference types="node" />

import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import StarMap from './StarMap.svelte'
import StarMapTestHost from './StarMap.test-host.svelte'
import type { SpaceModel } from './model'
import { StarMap as Renderer } from './starmap/starmap'
import { structureSignature, type LayoutNode } from './starmap/layout'
import type {
  LayoutLoad,
  LayoutOutcome,
  LayoutPoints,
  LayoutRequester,
} from './starmap/layout-loader'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) {
    await unmount(component)
  }
  document.body.innerHTML = ''
  document.head.querySelectorAll('[data-test-app-css]').forEach((style) => style.remove())
  document.documentElement.style.removeProperty('--map-background')
  vi.useRealTimers()
  vi.restoreAllMocks()
})

function pointsFor(nodes: LayoutNode[]): LayoutPoints {
  return Object.fromEntries(
    nodes.map((node, index) => [node.num, { x: 100 + index * 80, y: 200 + index * 40 }]),
  )
}

const immediateLayout: LayoutRequester = {
  load(nodes): LayoutLoad {
    return {
      kind: 'cached',
      signature: structureSignature(nodes),
      points: pointsFor(nodes),
    }
  },
}

interface ControlledRequest {
  nodes: LayoutNode[]
  cancelCalls: number
  resolve(outcome: LayoutOutcome): void
}

class ControlledLayout implements LayoutRequester {
  requests: ControlledRequest[] = []
  resolveCancellation = true

  load(nodes: LayoutNode[]): LayoutLoad {
    let resolveOutcome!: (outcome: LayoutOutcome) => void
    const request: ControlledRequest = {
      nodes,
      cancelCalls: 0,
      resolve: (outcome) => resolveOutcome(outcome),
    }
    const result = new Promise<LayoutOutcome>((resolve) => {
      resolveOutcome = resolve
    })
    this.requests.push(request)
    return {
      kind: 'pending',
      signature: structureSignature(nodes),
      result,
      cancel: () => {
        request.cancelCalls++
        if (this.resolveCancellation) resolveOutcome({ kind: 'cancelled' })
      },
    }
  }
}

async function settle(): Promise<void> {
  await Promise.resolve()
  await Promise.resolve()
  flushSync()
}

function space(number: number): SpaceModel {
  return {
    id: 'teloverge-stellr',
    repo: 'teloverge/stellr',
    name: 'stellr',
    synced_at: null,
    stale: false,
    error: null,
    stars: [
      {
        number,
        parent_issue: null,
        title: `Issue ${number}`,
        status: 'frontier',
        blocked_by: [],
        milestone: null,
        labels: [],
        assignees: [],
        url: `https://github.com/teloverge/stellr/issues/${number}`,
        body: '',
      },
    ],
  }
}

describe('StarMap wrapper', () => {
  it('defines pure black as the chart background in every shell mode', () => {
    const style = document.createElement('style')
    style.dataset.testAppCss = ''
    style.textContent = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8').replace(
      '@import "tailwindcss";',
      '',
    )
    document.head.appendChild(style)

    for (const theme of ['light', 'dark']) {
      document.documentElement.dataset.theme = theme
      expect(
        getComputedStyle(document.documentElement).getPropertyValue('--map-background').trim(),
      ).toBe('#000')
    }
  })

  it('sets the initial renderer background from the mounted document token', () => {
    document.documentElement.style.setProperty('--map-background', 'rgb(12, 34, 56)')
    const setBackground = vi.spyOn(Renderer.prototype, 'setBackground')
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMap, {
      target,
      props: { space: space(42), layout: immediateLayout },
    })
    mounted.push(component)
    flushSync()

    expect(setBackground).toHaveBeenCalledWith('rgb(12, 34, 56)')
  })

  it('feeds reactive space prop updates to the renderer', () => {
    const setModel = vi.spyOn(Renderer.prototype, 'setModel')
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMapTestHost, {
      target,
      props: { initialSpace: space(42), layout: immediateLayout },
    })
    mounted.push(component)
    flushSync()

    component.updateSpace(space(99))
    flushSync()

    expect(setModel).toHaveBeenLastCalledWith(
      [expect.objectContaining({ num: 99, slug: '99', title: 'Issue 99' })],
      {},
      null,
      expect.objectContaining({ 99: expect.any(Object) }),
    )
  })

  it('passes the current conversation issue to the renderer', () => {
    const setModel = vi.spyOn(Renderer.prototype, 'setModel')
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMap, {
      target,
      props: { space: space(42), currentIssue: 14, layout: immediateLayout },
    })
    mounted.push(component)
    flushSync()

    expect(setModel).toHaveBeenLastCalledWith(
      [expect.objectContaining({ num: 42 })],
      {},
      14,
      expect.objectContaining({ 42: expect.any(Object) }),
    )
  })

  it('does not echo a routed selection back through the user-selection callback', () => {
    const target = document.createElement('div')
    document.body.appendChild(target)
    const selected: number[] = []

    const component = mount(StarMap, {
      target,
      props: {
        space: space(42),
        selectedIssue: 42,
        select: (number) => selected.push(number),
        layout: immediateLayout,
      },
    })
    mounted.push(component)
    flushSync()

    expect(selected).toEqual([])
  })

  it('clears before restoring the same routed issue when the space changes', () => {
    const select = vi.spyOn(Renderer.prototype, 'select')
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMapTestHost, {
      target,
      props: { initialSpace: space(42), initialSelectedIssue: 42, layout: immediateLayout },
    })
    mounted.push(component)
    flushSync()

    select.mockClear()
    component.updateSpace({ ...space(42), id: 'another-space' })
    flushSync()

    expect(select.mock.calls).toEqual([[null], [42]])
  })

  it('mounts the canvas island and forwards a selected issue number', () => {
    const target = document.createElement('div')
    document.body.appendChild(target)
    const selected: number[] = []

    const component = mount(StarMap, {
      target,
      props: {
        space: space(42),
        select: (number) => selected.push(number),
        layout: immediateLayout,
      },
    })
    mounted.push(component)

    const host = target.querySelector<HTMLElement>('.star-map')!
    Object.defineProperty(host, 'clientWidth', { value: 1000, configurable: true })
    Object.defineProperty(host, 'clientHeight', { value: 700, configurable: true })
    flushSync()

    const canvas = host.querySelector('canvas')!
    canvas.dispatchEvent(new MouseEvent('mousedown', { clientX: 500, clientY: 350, bubbles: true }))
    canvas.dispatchEvent(new MouseEvent('mouseup', { clientX: 500, clientY: 350, bubbles: true }))

    expect(selected).toEqual([42])
  })

  it('shows timed first-load progress while a layout is pending', async () => {
    vi.useFakeTimers()
    const layout = new ControlledLayout()
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMap, { target, props: { space: space(42), layout } })
    mounted.push(component)
    flushSync()

    expect(target.textContent).toContain('Charting stellr...')
    expect(target.textContent).toContain('0 seconds elapsed.')
    expect(target.querySelector('.star-map')?.getAttribute('aria-hidden')).toBe('true')

    vi.advanceTimersByTime(2_000)
    flushSync()
    expect(target.textContent).toContain('2 seconds elapsed.')
  })

  it('applies ready coordinates and reports the successfully charted project', async () => {
    const layout = new ControlledLayout()
    const ready: string[] = []
    const setModel = vi.spyOn(Renderer.prototype, 'setModel')
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMap, {
      target,
      props: { space: space(42), layout, ready: (spaceId) => ready.push(spaceId) },
    })
    mounted.push(component)
    flushSync()

    const points = { 42: { x: 12, y: 34 } }
    layout.requests[0].resolve({ kind: 'ready', points })
    await settle()

    expect(setModel).toHaveBeenLastCalledWith(
      [expect.objectContaining({ num: 42 })],
      {},
      null,
      points,
    )
    expect(ready).toEqual(['teloverge-stellr'])
    expect(target.textContent).not.toContain('First load may take a moment.')
    expect(target.querySelector('.star-map')?.getAttribute('aria-hidden')).toBeNull()
  })

  it('cancels the active request and reports the project id', async () => {
    const layout = new ControlledLayout()
    const cancelled: string[] = []
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMap, {
      target,
      props: { space: space(42), layout, cancelled: (spaceId) => cancelled.push(spaceId) },
    })
    mounted.push(component)
    flushSync()

    target.querySelector<HTMLButtonElement>('button[aria-label^="Cancel layout"]')!.click()
    await settle()

    expect(layout.requests[0].cancelCalls).toBe(1)
    expect(cancelled).toEqual(['teloverge-stellr'])
    expect(target.textContent).toContain('Layout canceled')
    expect(target.textContent).toContain('Retry')
  })

  it('ignores a superseded result and applies only the current project', async () => {
    const layout = new ControlledLayout()
    layout.resolveCancellation = false
    const ready: string[] = []
    const setModel = vi.spyOn(Renderer.prototype, 'setModel')
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMapTestHost, {
      target,
      props: {
        initialSpace: space(42),
        layout,
        ready: (spaceId) => ready.push(spaceId),
      },
    })
    mounted.push(component)
    flushSync()

    component.updateSpace({ ...space(99), id: 'teloverge-other', name: 'other' })
    flushSync()
    expect(layout.requests[0].cancelCalls).toBe(1)

    layout.requests[0].resolve({ kind: 'ready', points: { 42: { x: 1, y: 2 } } })
    await settle()
    expect(setModel).not.toHaveBeenCalled()

    layout.requests[1].resolve({ kind: 'ready', points: { 99: { x: 3, y: 4 } } })
    await settle()
    expect(setModel).toHaveBeenCalledOnce()
    expect(setModel.mock.calls[0]?.[0][0]?.num).toBe(99)
    expect(ready).toEqual(['teloverge-other'])
  })

  it('shows an error with Retry after layout failure and starts fresh work on retry', async () => {
    const layout = new ControlledLayout()
    const failures: Array<[string, string]> = []
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMap, {
      target,
      props: {
        space: space(42),
        layout,
        failed: (spaceId, message) => failures.push([spaceId, message]),
      },
    })
    mounted.push(component)
    flushSync()

    layout.requests[0].resolve({ kind: 'failed', message: 'worker exploded' })
    await settle()
    expect(failures).toEqual([['teloverge-stellr', 'worker exploded']])
    expect(target.textContent).toContain('Could not chart stellr')
    expect(target.textContent).toContain('worker exploded')

    target.querySelector<HTMLButtonElement>('button')!.click()
    flushSync()
    expect(layout.requests).toHaveLength(2)
    expect(target.textContent).toContain('0 seconds elapsed.')
  })

  it('cancels pending work and clears its stopwatch when destroyed', async () => {
    vi.useFakeTimers()
    const layout = new ControlledLayout()
    const target = document.createElement('div')
    document.body.appendChild(target)
    const clearInterval = vi.spyOn(window, 'clearInterval')

    const component = mount(StarMap, { target, props: { space: space(42), layout } })
    flushSync()

    await unmount(component)

    expect(layout.requests[0].cancelCalls).toBe(1)
    expect(clearInterval).toHaveBeenCalled()
  })
})

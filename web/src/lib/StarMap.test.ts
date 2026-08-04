/// <reference types="node" />

import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import StarMap from './StarMap.svelte'
import StarMapTestHost from './StarMap.test-host.svelte'
import type { SpaceModel } from './model'
import { StarMap as Renderer } from './starmap/starmap'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) {
    await unmount(component)
  }
  document.body.innerHTML = ''
  document.head.querySelectorAll('[data-test-app-css]').forEach((style) => style.remove())
  document.documentElement.style.removeProperty('--map-background')
  vi.restoreAllMocks()
})

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
      props: { space: space(42) },
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
      props: { initialSpace: space(42) },
    })
    mounted.push(component)
    flushSync()

    component.updateSpace(space(99))
    flushSync()

    expect(setModel).toHaveBeenLastCalledWith(
      [expect.objectContaining({ num: 99, slug: '99', title: 'Issue 99' })],
      {},
      null,
    )
  })

  it('passes the current conversation issue to the renderer', () => {
    const setModel = vi.spyOn(Renderer.prototype, 'setModel')
    const target = document.createElement('div')
    document.body.appendChild(target)

    const component = mount(StarMap, {
      target,
      props: { space: space(42), currentIssue: 14 },
    })
    mounted.push(component)
    flushSync()

    expect(setModel).toHaveBeenLastCalledWith(
      [expect.objectContaining({ num: 42 })],
      {},
      14,
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
      props: { initialSpace: space(42), initialSelectedIssue: 42 },
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
      props: { space: space(42), select: (number) => selected.push(number) },
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
})

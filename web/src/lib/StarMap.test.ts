import { afterEach, describe, expect, it } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import StarMap from './StarMap.svelte'
import type { SpaceModel } from './model'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) {
    await unmount(component)
  }
  document.body.innerHTML = ''
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

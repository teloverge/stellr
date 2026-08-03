import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import AppearanceMenu from './AppearanceMenu.svelte'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

describe('appearance settings', () => {
  it('exposes System, Light, and Dark as an accessible single-choice group', async () => {
    const select = vi.fn(async () => undefined)
    const target = document.createElement('div')
    document.body.appendChild(target)
    mounted.push(mount(AppearanceMenu, { target, props: { preference: 'system', select } }))
    flushSync()

    target.querySelector<HTMLButtonElement>('button[aria-label="Appearance"]')!.click()
    flushSync()

    const choices = [...target.querySelectorAll<HTMLElement>('[role="radio"]')]
    expect(choices.map((choice) => choice.textContent?.trim())).toEqual(['System', 'Light', 'Dark'])
    expect(choices.map((choice) => choice.getAttribute('aria-checked'))).toEqual([
      'true',
      'false',
      'false',
    ])

    choices[2].click()
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(select).toHaveBeenCalledWith('dark')
  })
})

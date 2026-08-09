import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, unmount, type ComponentProps } from 'svelte'
import LayoutTransition from './LayoutTransition.svelte'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component)
  document.body.innerHTML = ''
})

function render(props: ComponentProps<typeof LayoutTransition>): HTMLElement {
  const target = document.createElement('div')
  document.body.appendChild(target)
  mounted.push(mount(LayoutTransition, { target, props }))
  return target
}

describe('LayoutTransition', () => {
  it('shows project-specific first-load progress without announcing every stopwatch tick', () => {
    const cancel = vi.fn()
    const target = render({
      kind: 'loading',
      projectName: 'Evolve',
      elapsedSeconds: 12,
      cancel,
    })

    expect(target.textContent).toContain('Charting Evolve...')
    expect(target.textContent).toContain('First load may take a moment.')
    expect(target.textContent).toContain('12 seconds elapsed.')
    expect(target.querySelector('[role="status"]')).not.toBeNull()
    expect(target.querySelector('[data-elapsed]')?.getAttribute('aria-live')).toBe('off')

    const button = target.querySelector<HTMLButtonElement>(
      'button[aria-label="Cancel layout for Evolve"]',
    )!
    button.click()
    expect(cancel).toHaveBeenCalledOnce()
  })

  it('shows retry actions for canceled and failed layouts', async () => {
    const retryCanceled = vi.fn()
    const canceled = render({ kind: 'cancelled', projectName: 'Evolve', retry: retryCanceled })
    expect(canceled.textContent).toContain('Layout canceled')
    canceled.querySelector<HTMLButtonElement>('button')!.click()
    expect(retryCanceled).toHaveBeenCalledOnce()

    await unmount(mounted.shift()!)
    canceled.remove()

    const retryFailed = vi.fn()
    const failed = render({
      kind: 'error',
      projectName: 'Evolve',
      message: 'worker exploded',
      retry: retryFailed,
    })
    expect(failed.textContent).toContain('Could not chart Evolve')
    expect(failed.textContent).toContain('worker exploded')
    failed.querySelector<HTMLButtonElement>('button')!.click()
    expect(retryFailed).toHaveBeenCalledOnce()
  })
})

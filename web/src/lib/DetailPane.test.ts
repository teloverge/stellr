import { afterEach, describe, expect, it } from 'vitest'
import { mount, unmount } from 'svelte'
import DetailPane from './DetailPane.svelte'
import type { Star } from './model'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) {
    await unmount(component)
  }
  document.body.innerHTML = ''
})

function issue(overrides: Partial<Star> = {}): Star {
  return {
    number: 42,
    parent_issue: null,
    title: 'Fix the thing',
    status: 'frontier',
    blocked_by: [],
    milestone: 'M1',
    labels: ['ready-for-agent', 'frontend'],
    assignees: ['alice'],
    url: 'https://github.com/teloverge/stellr/issues/42',
    body: 'Body with **safe detail**.',
    ...overrides,
  }
}

function render(star: Star, close = () => undefined): HTMLElement {
  const target = document.createElement('div')
  document.body.appendChild(target)
  mounted.push(mount(DetailPane, { target, props: { star, close } }))
  return target
}

describe('DetailPane', () => {
  it('presents complete issue context and closes through its public callback', () => {
    let closed = false
    const target = render(issue(), () => {
      closed = true
    })

    expect(target.textContent).toContain('#42')
    expect(target.textContent).toContain('Fix the thing')
    expect(target.textContent).toContain('frontier')
    expect(target.textContent).toContain('M1')
    expect(target.textContent).toContain('ready-for-agent')
    expect(target.textContent).toContain('frontend')
    expect(target.textContent).toContain('@alice')
    expect(target.querySelector('strong')?.textContent).toBe('safe detail')

    const link = target.querySelector<HTMLAnchorElement>('a[href$="/issues/42"]')!
    expect(link.target).toBe('_blank')
    expect(link.rel).toBe('noreferrer')

    target
      .querySelector<HTMLButtonElement>('button[aria-label="Close issue details"]')!
      .dispatchEvent(new MouseEvent('click', { bubbles: true }))
    expect(closed).toBe(true)
  })

  it('omits metadata sections that have no values', () => {
    const target = render(issue({ milestone: null, labels: [], assignees: [], body: '' }))

    expect(target.textContent).not.toContain('Milestone')
    expect(target.textContent).not.toContain('Labels')
    expect(target.textContent).not.toContain('Assignees')
  })
})

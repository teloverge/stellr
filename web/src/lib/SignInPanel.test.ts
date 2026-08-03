import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushSync, mount, unmount } from 'svelte'
import SignInPanel from './SignInPanel.svelte'
import type { DeviceFlowStatus } from './native-auth'

const mounted: object[] = []

afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component)
  document.body.innerHTML = ''
  vi.restoreAllMocks()
})

function render(
  status: DeviceFlowStatus,
  begin = vi.fn(async () => undefined),
  cancel = vi.fn(async () => undefined),
): { target: HTMLElement; begin: typeof begin; cancel: typeof cancel } {
  const target = document.createElement('div')
  document.body.appendChild(target)
  mounted.push(mount(SignInPanel, { target, props: { status, begin, cancel } }))
  flushSync()
  return { target, begin, cancel }
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0))
  flushSync()
}

describe('native GitHub sign-in panel', () => {
  it('starts device authorization without asking for or rendering a token', async () => {
    const { target, begin } = render({ state: 'idle' })

    expect(target.textContent).toContain('Connect GitHub')
    expect(target.querySelector('input')).toBeNull()
    target.querySelector<HTMLButtonElement>('button')!.click()
    await settle()

    expect(begin).toHaveBeenCalledOnce()
  })

  it('shows only the operator-safe code, verification link, expiry, and cancel action', async () => {
    const { target, cancel } = render({
      state: 'pending',
      user_code: 'ABCD-EFGH',
      verification_uri: 'https://github.com/login/device',
      expires_in_seconds: 900,
      interval_seconds: 5,
    })

    expect(target.textContent).toContain('ABCD-EFGH')
    expect(target.textContent).toContain('15 minutes')
    expect(target.innerHTML).not.toContain('device_code')
    expect(target.innerHTML).not.toContain('access_token')
    expect(target.querySelector<HTMLAnchorElement>('a')?.href).toBe(
      'https://github.com/login/device',
    )

    target.querySelector<HTMLButtonElement>('button')!.click()
    await settle()
    expect(cancel).toHaveBeenCalledOnce()
  })

  it.each([
    ['denied', 'GitHub declined this request.'],
    ['expired', 'This sign-in code expired.'],
    ['cancelled', 'Sign-in was cancelled.'],
  ] as const)('offers an explicit retry after %s', async (state, message) => {
    const { target, begin } = render({ state })

    expect(target.textContent).toContain(message)
    expect(target.textContent).toContain('Try again')
    target.querySelector<HTMLButtonElement>('button')!.click()
    await settle()
    expect(begin).toHaveBeenCalledOnce()
  })

  it('explains GitHub slow-down while preserving the current code', () => {
    const { target } = render({
      state: 'slow_down',
      user_code: 'WXYZ-1234',
      verification_uri: 'https://github.com/login/device',
      expires_in_seconds: 900,
      interval_seconds: 10,
    })

    expect(target.textContent).toContain('WXYZ-1234')
    expect(target.textContent).toContain('GitHub asked Stellr to check less often')
  })

  it('keeps the live connection visible when durable credential storage fails', () => {
    const { target } = render({
      state: 'authorized',
      storage_warning: 'GitHub is connected for this run, but Windows Credential Manager failed.',
    })

    expect(target.textContent).toContain('GitHub is connected')
    expect(target.textContent).toContain('Windows Credential Manager failed')
  })
})

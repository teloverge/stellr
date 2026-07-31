import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Control } from './control.svelte'

class FakeWebSocket {
  static instances: FakeWebSocket[] = []

  onclose: ((event: CloseEvent) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  onopen: ((event: Event) => void) | null = null

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this)
  }

  close(): void {
    this.emitClose()
  }

  emitClose(): void {
    this.onclose?.(new CloseEvent('close'))
  }

  emitMessage(data: string): void {
    this.onmessage?.(new MessageEvent('message', { data }))
  }

  emitOpen(): void {
    this.onopen?.(new Event('open'))
  }
}

function setPageUrl(url: string): void {
  vi.stubGlobal('window', { location: new URL(url) })
}

describe('Control', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    FakeWebSocket.instances = []
    vi.stubGlobal('WebSocket', FakeWebSocket as unknown as typeof WebSocket)
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it('applies text snapshots, reports connection status, and reconnects after 500 ms', () => {
    const control = new Control()

    control.connect('ws://socket.example/ws/control')
    const first = FakeWebSocket.instances[0]
    expect(control.status).toBe('connecting')

    first.emitOpen()
    expect(control.status).toBe('open')

    first.emitMessage('{"spaces":[]}')
    expect(control.model).toEqual({ spaces: [] })

    first.emitClose()
    expect(control.status).toBe('closed')
    vi.advanceTimersByTime(499)
    expect(FakeWebSocket.instances).toHaveLength(1)

    vi.advanceTimersByTime(1)
    expect(FakeWebSocket.instances).toHaveLength(2)
    expect(FakeWebSocket.instances[1].url).toBe('ws://socket.example/ws/control')
    expect(control.status).toBe('connecting')
  })

  it('derives an http page URL and carries its token', () => {
    setPageUrl('http://stellr.test:4173/chart?token=page-secret&ignored=yes')

    new Control().connect()

    expect(FakeWebSocket.instances[0].url).toBe(
      'ws://stellr.test:4173/ws/control?token=page-secret',
    )
  })

  it('derives a secure WebSocket URL from an https page', () => {
    setPageUrl('https://stellr.test/chart')

    new Control().connect()

    expect(FakeWebSocket.instances[0].url).toBe('wss://stellr.test/ws/control')
  })

  it('carries the page token into a supplied WebSocket URL', () => {
    setPageUrl('https://stellr.test/?token=page-secret')

    new Control().connect('wss://socket.example/ws/control?existing=yes')

    expect(FakeWebSocket.instances[0].url).toBe(
      'wss://socket.example/ws/control?existing=yes&token=page-secret',
    )
  })

  it('ignores duplicate and stale close events instead of scheduling extra reconnects', () => {
    const control = new Control()
    control.connect('ws://socket.example/ws/control')
    const first = FakeWebSocket.instances[0]
    const staleClose = first.onclose

    staleClose?.(new CloseEvent('close'))
    staleClose?.(new CloseEvent('close'))
    expect(vi.getTimerCount()).toBe(1)

    vi.advanceTimersByTime(500)
    expect(FakeWebSocket.instances).toHaveLength(2)

    staleClose?.(new CloseEvent('close'))
    vi.advanceTimersByTime(500)
    expect(FakeWebSocket.instances).toHaveLength(2)
    expect(vi.getTimerCount()).toBe(0)
  })
})

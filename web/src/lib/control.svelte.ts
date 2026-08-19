import type { Model } from './model'

export type ConnectionStatus = 'connecting' | 'open' | 'closed' | 'unauthorized'
type SessionProbe = () => Promise<Response>

function probeSession(): Promise<Response> {
  return fetch('/api/model', { credentials: 'same-origin' })
}

export function pageIssue(): number | null {
  const raw = new URL(window.location.href).searchParams.get('issue')
  if (raw === null || !/^\d+$/.test(raw)) return null
  const issue = Number(raw)
  return Number.isSafeInteger(issue) && issue > 0 ? issue : null
}

export function takePageToken(): string | null {
  const page = new URL(window.location.href)
  const token = page.searchParams.get('token')
  if (token !== null) {
    page.searchParams.delete('token')
    window.history.replaceState(
      window.history.state,
      '',
      `${page.pathname}${page.search}${page.hash}`,
    )
  }
  return token
}

function controlUrl(token: string | null, url?: string): string {
  const target = url
    ? new URL(url)
    : new URL('/ws/control', window.location.href)

  if (!url) {
    target.protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  }

  if (token !== null) {
    target.searchParams.set('token', token)
  }

  return target.toString()
}

export class Control {
  model = $state<Model | null>(null)
  revision = $state(0)
  status = $state<ConnectionStatus>('connecting')

  #reconnectTimer: ReturnType<typeof setTimeout> | null = null
  #socket: WebSocket | null = null
  #token: string | null
  #url: string | null = null

  #sessionProbe: SessionProbe

  constructor(
    token: string | null = takePageToken(),
    sessionProbe: SessionProbe = probeSession,
  ) {
    this.#token = token
    this.#sessionProbe = sessionProbe
  }

  connect(url?: string): void {
    const nextUrl = controlUrl(this.#token, url)
    this.#url = nextUrl
    this.#clearReconnect()

    const previous = this.#socket
    this.#socket = null
    if (previous !== null) {
      previous.onopen = null
      previous.onmessage = null
      previous.onclose = null
      previous.close()
    }

    this.#open(nextUrl)
  }

  destroy(): void {
    this.#url = null
    this.#clearReconnect()

    const socket = this.#socket
    this.#socket = null
    if (socket !== null) {
      socket.onopen = null
      socket.onmessage = null
      socket.onclose = null
      socket.close()
    }
    this.status = 'closed'
  }

  #clearReconnect(): void {
    if (this.#reconnectTimer !== null) {
      clearTimeout(this.#reconnectTimer)
      this.#reconnectTimer = null
    }
  }

  #open(url: string): void {
    this.status = 'connecting'
    const socket = new WebSocket(url)
    this.#socket = socket

    socket.onopen = () => {
      if (this.#socket === socket) {
        this.status = 'open'
      }
    }

    socket.onmessage = (event) => {
      if (this.#socket === socket && typeof event.data === 'string') {
        this.model = JSON.parse(event.data) as Model
        this.revision += 1
      }
    }

    socket.onclose = () => {
      if (this.#socket !== socket) {
        return
      }

      this.#socket = null
      this.status = 'closed'
      this.#clearReconnect()
      this.#reconnectTimer = setTimeout(() => {
        this.#reconnectTimer = null
        if (this.#socket === null && this.#url === url) {
          this.#open(url)
        }
      }, 500)
      void this.#detectExpiredSession(url)
    }
  }

  async #detectExpiredSession(url: string): Promise<void> {
    let response: Response
    try {
      response = await this.#sessionProbe()
    } catch {
      return
    }
    if (response.status !== 401 || this.#url !== url) {
      return
    }

    this.#url = null
    this.#clearReconnect()
    const socket = this.#socket
    this.#socket = null
    if (socket !== null) {
      socket.onopen = null
      socket.onmessage = null
      socket.onclose = null
      socket.close()
    }
    this.status = 'unauthorized'
  }
}

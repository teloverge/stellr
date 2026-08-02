import type { Model } from './model'

type ConnectionStatus = 'connecting' | 'open' | 'closed'

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
  status = $state<ConnectionStatus>('connecting')

  #reconnectTimer: ReturnType<typeof setTimeout> | null = null
  #socket: WebSocket | null = null
  #token: string | null
  #url: string | null = null

  constructor(token: string | null = takePageToken()) {
    this.#token = token
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
    }
  }
}

export interface RouteState {
  space: string | null
  issue: number | null
}

function positiveIssue(raw: string | null): number | null {
  if (raw === null || !/^\d+$/.test(raw)) return null
  const issue = Number(raw)
  return Number.isSafeInteger(issue) && issue > 0 ? issue : null
}

export function parseRouteHash(hash: string): RouteState {
  const params = new URLSearchParams(hash.startsWith('#') ? hash.slice(1) : hash)
  const rawSpace = params.get('s')

  return {
    space: rawSpace === null || rawSpace.length === 0 ? null : rawSpace,
    issue: positiveIssue(params.get('i')),
  }
}

export function formatRouteHash(space: string | null, issue: number | null = null): string {
  if (space === null || space.length === 0) return ''

  const params = new URLSearchParams()
  params.set('s', space)
  if (Number.isSafeInteger(issue) && issue !== null && issue > 0) {
    params.set('i', String(issue))
  }
  return `#${params.toString()}`
}

export class Route {
  space = $state<string | null>(null)
  issue = $state<number | null>(null)

  readonly #target: Window
  readonly #update: () => void

  constructor(target: Window = window) {
    this.#target = target
    this.#update = () => this.#apply(target.location.hash)
    this.#target.addEventListener('hashchange', this.#update)
    this.#update()
  }

  go(space: string | null, issue: number | null = null): void {
    this.#target.location.hash = formatRouteHash(space, issue)
    this.#apply(this.#target.location.hash)
  }

  destroy(): void {
    this.#target.removeEventListener('hashchange', this.#update)
  }

  #apply(hash: string): void {
    const route = parseRouteHash(hash)
    this.space = route.space
    this.issue = route.issue
  }
}

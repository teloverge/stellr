import { structureSignature, type LayoutNode, type Point } from './layout'

export type LayoutPoints = Record<number, Point>

export type LayoutOutcome =
  | { kind: 'ready'; points: LayoutPoints }
  | { kind: 'cancelled' }
  | { kind: 'failed'; message: string }

export type LayoutLoad =
  | { kind: 'cached'; signature: string; points: LayoutPoints }
  | {
      kind: 'pending'
      signature: string
      result: Promise<LayoutOutcome>
      cancel(): void
    }

export interface LayoutRequester {
  load(nodes: LayoutNode[]): LayoutLoad
}

export interface LayoutWorkerPort {
  onmessage: ((event: MessageEvent<unknown>) => void) | null
  onerror: ((event: ErrorEvent) => void) | null
  postMessage(message: { nodes: LayoutNode[] }): void
  terminate(): void
}

function clonePoints(points: LayoutPoints): LayoutPoints {
  return Object.fromEntries(
    Object.entries(points).map(([number, point]) => [number, { ...point }]),
  ) as LayoutPoints
}

function readyPoints(nodes: LayoutNode[], value: unknown): LayoutPoints | string {
  if (typeof value !== 'object' || value === null) return 'Layout result is not an object.'
  const points = value as Record<number, Point | undefined>
  for (const node of nodes) {
    const point = points[node.num]
    if (point === undefined || !Number.isFinite(point.x) || !Number.isFinite(point.y)) {
      return `Layout result is missing finite coordinates for issue ${node.num}.`
    }
  }
  return points as LayoutPoints
}

function failureMessage(error: unknown): string {
  return error instanceof Error && error.message.length > 0
    ? error.message
    : 'Layout worker failed.'
}

export class LayoutLoader implements LayoutRequester {
  readonly #cache = new Map<string, LayoutPoints>()
  readonly #workerFactory: () => LayoutWorkerPort

  constructor(workerFactory: () => LayoutWorkerPort) {
    this.#workerFactory = workerFactory
  }

  load(nodes: LayoutNode[]): LayoutLoad {
    const signature = structureSignature(nodes)
    const cached = this.#cache.get(signature)
    if (cached !== undefined) {
      return { kind: 'cached', signature, points: clonePoints(cached) }
    }

    let worker: LayoutWorkerPort | null = null
    let settled = false
    let resolveOutcome!: (outcome: LayoutOutcome) => void
    const result = new Promise<LayoutOutcome>((resolve) => {
      resolveOutcome = resolve
    })
    const finish = (outcome: LayoutOutcome): void => {
      if (settled) return
      settled = true
      worker?.terminate()
      worker = null
      resolveOutcome(outcome)
    }

    try {
      worker = this.#workerFactory()
      worker.onmessage = (event) => {
        if (settled) return
        const message = event.data
        if (typeof message !== 'object' || message === null || !('kind' in message)) {
          finish({ kind: 'failed', message: 'Layout worker returned a malformed response.' })
          return
        }
        if (message.kind === 'failed') {
          const reported = 'message' in message ? message.message : undefined
          finish({
            kind: 'failed',
            message:
              typeof reported === 'string' && reported.length > 0
                ? reported
                : 'Layout worker failed.',
          })
          return
        }
        if (message.kind !== 'ready' || !('points' in message)) {
          finish({ kind: 'failed', message: 'Layout worker returned a malformed response.' })
          return
        }
        const validated = readyPoints(nodes, message.points)
        if (typeof validated === 'string') {
          finish({ kind: 'failed', message: validated })
          return
        }
        const stored = clonePoints(validated)
        this.#cache.set(signature, stored)
        finish({ kind: 'ready', points: clonePoints(stored) })
      }
      worker.onerror = (event) => {
        if (settled) return
        finish({
          kind: 'failed',
          message: event.message.length > 0 ? event.message : 'Layout worker failed.',
        })
      }
      worker.postMessage({ nodes })
    } catch (error) {
      finish({ kind: 'failed', message: failureMessage(error) })
    }

    return {
      kind: 'pending',
      signature,
      result,
      cancel: () => finish({ kind: 'cancelled' }),
    }
  }
}

export const browserLayoutLoader: LayoutRequester = new LayoutLoader(
  () =>
    new Worker(new URL('./layout.worker.ts', import.meta.url), {
      type: 'module',
    }) as LayoutWorkerPort,
)

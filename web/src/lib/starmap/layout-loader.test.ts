import { describe, expect, it } from 'vitest'
import {
  LayoutLoader,
  type LayoutLoad,
  type LayoutOutcome,
  type LayoutWorkerPort,
} from './layout-loader'
import type { LayoutNode } from './layout'

class ControlledWorker implements LayoutWorkerPort {
  onmessage: ((event: MessageEvent<unknown>) => void) | null = null
  onerror: ((event: ErrorEvent) => void) | null = null
  posted: unknown[] = []
  terminated = false

  postMessage(message: unknown): void {
    this.posted.push(message)
  }

  terminate(): void {
    this.terminated = true
  }

  emit(data: unknown): void {
    this.onmessage?.(new MessageEvent('message', { data }))
  }

  fail(message: string): void {
    this.onerror?.(new ErrorEvent('error', { message }))
  }
}

const nodes: LayoutNode[] = [
  { num: 1, title: 'Parent', blockedBy: [], parentIssue: null },
  { num: 2, title: 'Child', blockedBy: [1], parentIssue: 1 },
]

function pending(load: LayoutLoad): Extract<LayoutLoad, { kind: 'pending' }> {
  expect(load.kind).toBe('pending')
  return load as Extract<LayoutLoad, { kind: 'pending' }>
}

async function outcome(load: LayoutLoad): Promise<LayoutOutcome> {
  return pending(load).result
}

function setup(): { loader: LayoutLoader; workers: ControlledWorker[] } {
  const workers: ControlledWorker[] = []
  return {
    loader: new LayoutLoader(() => {
      const worker = new ControlledWorker()
      workers.push(worker)
      return worker
    }),
    workers,
  }
}

describe('LayoutLoader', () => {
  it('returns defensive cached coordinates after one successful layout', async () => {
    const { loader, workers } = setup()
    const first = pending(loader.load(nodes))

    workers[0].emit({
      kind: 'ready',
      points: { 1: { x: 10, y: 20 }, 2: { x: 30, y: 40 } },
    })
    const ready = await first.result
    expect(ready).toEqual({
      kind: 'ready',
      points: { 1: { x: 10, y: 20 }, 2: { x: 30, y: 40 } },
    })
    expect(workers[0].terminated).toBe(true)

    if (ready.kind !== 'ready') throw new Error('expected ready coordinates')
    ready.points[1].x = 999
    const cached = loader.load(nodes)
    expect(cached).toEqual({
      kind: 'cached',
      signature: first.signature,
      points: { 1: { x: 10, y: 20 }, 2: { x: 30, y: 40 } },
    })
    expect(workers).toHaveLength(1)

    if (cached.kind !== 'cached') throw new Error('expected cached coordinates')
    cached.points[2].y = 777
    expect(loader.load(nodes)).toEqual({
      kind: 'cached',
      signature: first.signature,
      points: { 1: { x: 10, y: 20 }, 2: { x: 30, y: 40 } },
    })
  })

  it('reuses a signature while starting fresh work for structure and orbit-title changes', async () => {
    const { loader, workers } = setup()
    const first = pending(loader.load(nodes))
    workers[0].emit({
      kind: 'ready',
      points: { 1: { x: 1, y: 2 }, 2: { x: 3, y: 4 } },
    })
    await first.result

    expect(loader.load(nodes).kind).toBe('cached')
    expect(
      loader.load(nodes.map((node) => (node.num === 2 ? { ...node, blockedBy: [] } : node))).kind,
    ).toBe('pending')
    expect(
      loader.load(nodes.map((node) => (node.num === 2 ? { ...node, title: 'Renamed child' } : node)))
        .kind,
    ).toBe('pending')
    expect(workers).toHaveLength(3)
  })

  it('terminates cancellation and never caches the canceled result', async () => {
    const { loader, workers } = setup()
    const first = pending(loader.load(nodes))

    first.cancel()

    await expect(first.result).resolves.toEqual({ kind: 'cancelled' })
    expect(workers[0].terminated).toBe(true)
    expect(loader.load(nodes).kind).toBe('pending')
    expect(workers).toHaveLength(2)
  })

  it('ignores a ready message that races after cancellation', async () => {
    const { loader, workers } = setup()
    const first = pending(loader.load(nodes))
    first.cancel()
    workers[0].emit({
      kind: 'ready',
      points: { 1: { x: 1, y: 2 }, 2: { x: 3, y: 4 } },
    })

    await expect(first.result).resolves.toEqual({ kind: 'cancelled' })
    expect(loader.load(nodes).kind).toBe('pending')
  })

  it('rejects malformed coordinates without caching them', async () => {
    const { loader, workers } = setup()
    const first = loader.load(nodes)
    workers[0].emit({ kind: 'ready', points: { 1: { x: 1, y: 2 } } })

    await expect(outcome(first)).resolves.toEqual({
      kind: 'failed',
      message: 'Layout result is missing finite coordinates for issue 2.',
    })
    expect(workers[0].terminated).toBe(true)
    expect(loader.load(nodes).kind).toBe('pending')
  })

  it('turns worker errors into typed failures', async () => {
    const { loader, workers } = setup()
    const first = loader.load(nodes)
    workers[0].fail('worker exploded')

    await expect(outcome(first)).resolves.toEqual({
      kind: 'failed',
      message: 'worker exploded',
    })
    expect(workers[0].terminated).toBe(true)
  })
})

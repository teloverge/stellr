import { computeLayout, type LayoutNode } from './layout'

interface LayoutWorkerScope {
  onmessage: ((event: MessageEvent<{ nodes: LayoutNode[] }>) => void) | null
  postMessage(message: unknown): void
}

const worker = self as unknown as LayoutWorkerScope

worker.onmessage = (event) => {
  try {
    worker.postMessage({ kind: 'ready', points: computeLayout(event.data.nodes) })
  } catch (error) {
    worker.postMessage({ kind: 'failed', message: String(error) })
  }
}

import type { Ticket } from './model'

export interface Focus {
  current: number | null
  ready: number[]
  pathNodes: Set<number>
  pathEdges: Set<string>
  emphasized: Set<number>
}

export function edgeKey(from: number, to: number): string {
  return `${from}>${to}`
}

function isClosed(ticket: Ticket): boolean {
  return ticket.status === 'resolved' || ticket.status === 'out_of_scope'
}

function route(
  start: number,
  current: number,
  byNumber: Map<number, Ticket>,
  dependents: Map<number, number[]>,
): number[] | null {
  const queue = [start]
  const visited = new Set([start])
  const parent = new Map<number, number>()

  while (queue.length > 0) {
    const number = queue.shift()!
    if (number === current) {
      const path = [number]
      while (path[0] !== start) path.unshift(parent.get(path[0])!)
      return path
    }

    for (const next of dependents.get(number) ?? []) {
      if (visited.has(next)) continue
      const ticket = byNumber.get(next)
      if (!ticket || (next !== current && isClosed(ticket))) continue
      visited.add(next)
      parent.set(next, number)
      queue.push(next)
    }
  }

  return null
}

export function analyzeFocus(tickets: Ticket[], requestedCurrent: number | null): Focus {
  const byNumber = new Map(tickets.map((ticket) => [ticket.num, ticket]))
  const current = requestedCurrent !== null && byNumber.has(requestedCurrent)
    ? requestedCurrent
    : null
  const actionable = tickets
    .filter((ticket) => ticket.readyForAgent === true)
    .map((ticket) => ticket.num)
    .sort((a, b) => a - b)
  const dependents = new Map<number, number[]>()

  for (const ticket of tickets) {
    for (const blocker of ticket.blockedBy) {
      const list = dependents.get(blocker) ?? []
      list.push(ticket.num)
      dependents.set(blocker, list)
    }
  }
  for (const list of dependents.values()) list.sort((a, b) => a - b)

  const pathNodes = new Set<number>()
  const pathEdges = new Set<string>()
  const onPath: number[] = []

  if (current !== null) {
    for (const ready of actionable) {
      const path = route(ready, current, byNumber, dependents)
      if (path === null) continue
      onPath.push(ready)
      for (const number of path) pathNodes.add(number)
      for (let index = 1; index < path.length; index++) {
        pathEdges.add(edgeKey(path[index - 1], path[index]))
      }
    }
  }

  const pathReady = new Set(onPath)
  const ready = [...onPath, ...actionable.filter((number) => !pathReady.has(number))]
  const emphasized = new Set<number>()
  if (current !== null) emphasized.add(current)
  for (const number of ready) emphasized.add(number)
  for (const number of pathNodes) emphasized.add(number)

  return { current, ready, pathNodes, pathEdges, emphasized }
}

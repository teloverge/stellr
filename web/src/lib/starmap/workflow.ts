export type WorkflowRole = 'dependency' | 'entry' | 'sequence' | 'return'

export interface WorkflowNode {
  num: number
  blockedBy: number[]
  parentIssue: number | null
}

export interface WorkflowEdge {
  from: number
  to: number
  roles: WorkflowRole[]
  child: number | null
}

const ROLE_ORDER: WorkflowRole[] = ['dependency', 'entry', 'sequence', 'return']

export function workflowEdges(nodes: WorkflowNode[]): WorkflowEdge[] {
  const present = new Set(nodes.map((node) => node.num))
  const edges = new Map<string, WorkflowEdge>()

  function addEdge(from: number, to: number, role: WorkflowRole, child: number | null): void {
    const key = `${from}>${to}`
    const current = edges.get(key)
    if (current) {
      if (!current.roles.includes(role)) current.roles.push(role)
      if (current.child === null && child !== null) current.child = child
      return
    }
    edges.set(key, { from, to, roles: [role], child })
  }

  for (const node of nodes) {
    for (const blocker of node.blockedBy) {
      if (present.has(blocker)) addEdge(blocker, node.num, 'dependency', null)
    }
  }

  const childrenByParent = new Map<number, WorkflowNode[]>()
  for (const node of nodes) {
    const parent = node.parentIssue
    if (parent === null || parent === node.num || !present.has(parent)) continue
    const children = childrenByParent.get(parent) ?? []
    children.push(node)
    childrenByParent.set(parent, children)
  }

  for (const [parent, children] of childrenByParent) {
    const siblings = new Set(children.map((child) => child.num))
    for (const child of children) {
      const hasIncomingSibling = child.blockedBy.some((blocker) => blocker !== child.num && siblings.has(blocker))
      if (!hasIncomingSibling) addEdge(parent, child.num, 'entry', child.num)

      const hasOutgoingSibling = children.some(
        (sibling) => sibling.num !== child.num && sibling.blockedBy.includes(child.num),
      )
      if (!hasOutgoingSibling) addEdge(child.num, parent, 'return', child.num)

      for (const blocker of child.blockedBy) {
        if (blocker !== child.num && siblings.has(blocker)) {
          addEdge(blocker, child.num, 'sequence', child.num)
        }
      }
    }
  }

  return [...edges.values()]
    .map((edge) => ({ ...edge, roles: ROLE_ORDER.filter((role) => edge.roles.includes(role)) }))
    .sort((left, right) => left.from - right.from || left.to - right.to)
}

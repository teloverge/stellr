export interface ParentTopologyNode {
  num: number
  parentIssue: number | null
}

export function validOrbitNodeNumbers(nodes: ParentTopologyNode[]): Set<number> {
  const byNumber = new Map(nodes.map((node) => [node.num, node]))
  const state = new Map<number, 'visiting' | 'valid' | 'invalid'>()

  const terminatesAtRoot = (number: number): boolean => {
    const prior = state.get(number)
    if (prior === 'valid') return true
    if (prior === 'invalid' || prior === 'visiting') return false
    const node = byNumber.get(number)
    if (!node) return false
    if (node.parentIssue === null) {
      state.set(number, 'valid')
      return true
    }
    if (node.parentIssue === number || !byNumber.has(node.parentIssue)) {
      state.set(number, 'invalid')
      return false
    }

    state.set(number, 'visiting')
    const valid = terminatesAtRoot(node.parentIssue)
    state.set(number, valid ? 'valid' : 'invalid')
    return valid
  }

  const valid = new Set<number>()
  for (const node of nodes) {
    if (node.parentIssue !== null && terminatesAtRoot(node.num)) valid.add(node.num)
  }
  return valid
}

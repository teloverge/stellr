// The star-map palette expresses work priority, while session liveness remains
// an additive moon/mark overlay. Keeping the palette keyed by the derived
// priority prevents transport status from leaking into rendering decisions.

import type { WorkPriority } from './priority'

export type VisualState = WorkPriority

export interface StarStyle {
  core: string
  glow: string
  r: number
  minScreen: number
  gr: number
  solid: boolean
}

export const STAR: Record<VisualState, StarStyle> = {
  attention: { core: '#ffd873', glow: '#ffb020', r: 12, minScreen: 10, gr: 52, solid: true },
  doing_now: { core: '#ffd873', glow: '#ffb020', r: 12, minScreen: 10, gr: 49, solid: true },
  my_next: { core: '#8ad8ff', glow: '#2f9be0', r: 11, minScreen: 8.5, gr: 49, solid: true },
  my_future: { core: '#8ad8ff', glow: '#2f9be0', r: 10, minScreen: 7, gr: 42, solid: false },
  available_next: { core: '#8ed7ac', glow: '#3b9f68', r: 10, minScreen: 7, gr: 42, solid: false },
  team_work: { core: '#b9a7ee', glow: '#775dc1', r: 9, minScreen: 6, gr: 36, solid: true },
  planning: { core: '#aaa0bd', glow: '#716884', r: 5.625, minScreen: 0, gr: 20, solid: false },
  resolved: { core: '#b9d6c4', glow: '#5b9077', r: 6.75, minScreen: 0, gr: 24, solid: true },
  out_of_scope: { core: '#948da4', glow: '#6b6478', r: 5.625, minScreen: 0, gr: 18, solid: false },
}

export const LABEL: Record<VisualState, string> = {
  attention: '#ffe6a0',
  doing_now: '#ffe6a0',
  my_next: '#b3e5ff',
  my_future: '#b3e5ff',
  available_next: '#b9eccd',
  team_work: '#d6ccf6',
  planning: '#c8bfd7',
  resolved: '#a2c1ac',
  out_of_scope: '#a89fb2',
}

export const SESSION_HUE = {
  session: '#ffd873',
  gold: '#ffe6a0',
  dead: '#6b7280',
} as const

export function hexA(hex: string, a: number): string {
  const r = parseInt(hex.slice(1, 3), 16),
    g = parseInt(hex.slice(3, 5), 16),
    b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r},${g},${b},${a})`
}

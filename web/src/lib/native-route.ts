import { invoke } from '@tauri-apps/api/core'
import type { addSpace } from './api'

export interface NativeRouteTarget {
  space_id: string
  repo: string
  path: string | null
  issue: number | null
}

export type NativeRouteEvent =
  | { state: 'target'; target: NativeRouteTarget }
  | { state: 'error'; message: string }

export interface NativeRouteOutcome {
  route: { space: string; issue: number | null } | null
  error: string | null
}

export function takeNativeRouteEvent(): Promise<NativeRouteEvent | null> {
  return invoke<NativeRouteEvent | null>('take_route_event')
}

export async function applyNativeRouteEvent(
  event: NativeRouteEvent,
  existingSpaceIds: string[],
  addRequest: typeof addSpace,
): Promise<NativeRouteOutcome> {
  if (event.state === 'error') return { route: null, error: event.message }

  const target = event.target
  if (!existingSpaceIds.includes(target.space_id)) {
    try {
      const response = await addRequest(
        target.path === null ? { repo: target.repo } : { path: target.path },
      )
      if (!response.ok && response.status !== 409) {
        return {
          route: null,
          error: (await response.text()) || `Could not open ${target.repo}`,
        }
      }
    } catch (error) {
      return { route: null, error: String(error) }
    }
  }

  return {
    route: { space: target.space_id, issue: target.issue },
    error: null,
  }
}

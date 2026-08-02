import type { SpaceModel } from '../model'
import type { Ticket } from './model'

export function toRendererModel(space: SpaceModel): Ticket[] {
  return space.stars.map((star) => ({
    num: star.number,
    slug: String(star.number),
    title: star.title,
    type: 'issue',
    status: star.status,
    blockedBy: [...star.blocked_by],
    frontier: star.status === 'frontier',
    readyForAgent:
      star.status === 'frontier' &&
      star.labels.some((label) => label.toLowerCase() === 'ready-for-agent'),
  }))
}

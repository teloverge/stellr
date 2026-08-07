import type { SpaceModel } from '../model'
import type { Ticket } from './model'
import { deriveWorkPriority } from './work-priority'

export function toRendererModel(space: SpaceModel): Ticket[] {
  return space.stars.map((star) => {
    const workPriority = deriveWorkPriority(star)
    return {
      num: star.number,
      slug: String(star.number),
      title: star.title,
      type: 'issue',
      status: star.status,
      blockedBy: [...star.blocked_by],
      parentIssue: star.parent_issue,
      frontier: star.status === 'frontier',
      readyForAgent: workPriority === 'ready',
      workPriority,
    }
  })
}

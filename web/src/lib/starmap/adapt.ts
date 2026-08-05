import type { SpaceModel } from '../model'
import type { TemporalSpace } from '../history'
import type { Ticket } from './model'

export function toRendererModel(space: SpaceModel | TemporalSpace): Ticket[] {
  return space.stars.map((star) => {
    const liveStatus = 'live_status' in star ? star.live_status : star.status
    return {
      num: star.number,
      slug: String(star.number),
      title: star.title,
      type: 'issue',
      status: star.status,
      blockedBy: [...star.blocked_by],
      parentIssue: star.parent_issue,
      frontier: star.status === 'frontier',
      readyForAgent:
        liveStatus === 'frontier' &&
        star.labels.some((label) => label.toLowerCase() === 'ready-for-agent'),
      visible: 'temporal_visible' in star ? star.temporal_visible : true,
      focusStatus: liveStatus,
      milestone: star.milestone,
    }
  })
}

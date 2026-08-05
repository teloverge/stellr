import type { SpaceModel } from '../model'
import type { TemporalSpace } from '../history'
import type { Ticket } from './model'

export function toRendererModel(space: SpaceModel | TemporalSpace): Ticket[] {
  const historical = 'temporal_active' in space && space.temporal_active
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
      frontier: !historical && star.status === 'frontier',
      readyForAgent:
        !historical &&
        liveStatus === 'frontier' &&
        star.labels.some((label) => label.toLowerCase() === 'ready-for-agent'),
      visible: 'temporal_visible' in star ? star.temporal_visible : true,
      focusStatus: historical ? star.status : liveStatus,
      milestone: star.milestone,
      historical,
    }
  })
}

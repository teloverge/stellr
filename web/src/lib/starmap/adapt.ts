import type { SpaceModel } from '../model'
import type { TemporalSpace } from '../history'
import type { Ticket } from './model'
import { deriveWorkPriority } from './work-priority'

export function toRendererModel(space: SpaceModel | TemporalSpace): Ticket[] {
  const historical = 'temporal_active' in space && space.temporal_active
  const viewerLogin = space.viewer_login?.toLowerCase()
  return space.stars.map((star) => {
    const liveStatus = 'live_status' in star ? star.live_status : star.status
    const workPriority = historical
      ? undefined
      : deriveWorkPriority({
          status: liveStatus,
          labels: star.labels,
          assignees: star.assignees,
        })
    return {
      num: star.number,
      slug: String(star.number),
      title: star.title,
      type: 'issue',
      status: star.status,
      blockedBy: [...star.blocked_by],
      parentIssue: star.parent_issue,
      frontier: !historical && liveStatus === 'frontier',
      readyForAgent:
        !historical && (star.ready_for_agent === true || workPriority === 'ready'),
      workPriority,
      blocked: !historical && (star.blocked ?? liveStatus === 'blocked'),
      assignedToViewer:
        !historical &&
        viewerLogin !== undefined &&
        star.assignees.some((assignee) => assignee.toLowerCase() === viewerLogin),
      visible: 'temporal_visible' in star ? star.temporal_visible : true,
      focusStatus: historical ? star.status : liveStatus,
      milestone: star.milestone,
      historical,
    }
  })
}

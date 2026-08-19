import type { SpaceModel } from '../model'
import type { TemporalSpace } from '../history'
import type { Ticket } from './model'

export function toRendererModel(space: SpaceModel | TemporalSpace): Ticket[] {
  const historical = 'temporal_active' in space && space.temporal_active
  const viewerLogin = space.viewer_login?.toLowerCase()
  return space.stars.map((star) => {
    const liveStatus = 'live_status' in star ? star.live_status : star.status
    const legacyReadyForAgent =
      liveStatus === 'frontier' &&
      star.labels.some((label) => label.toLowerCase() === 'ready-for-agent')
    return {
      num: star.number,
      slug: String(star.number),
      title: star.title,
      type: 'issue',
      status: star.status,
      blockedBy: [...star.blocked_by],
      parentIssue: star.parent_issue,
      frontier: !historical && liveStatus === 'frontier',
      readyForAgent: !historical && (star.ready_for_agent ?? legacyReadyForAgent),
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

import type { SpaceModel } from '../model'
import type { Ticket } from './model'

export function toRendererModel(space: SpaceModel): Ticket[] {
  const viewerLogin = space.viewer_login?.toLowerCase()
  return space.stars.map((star) => {
    const legacyReadyForAgent =
      star.status === 'frontier' &&
      star.labels.some((label) => label.toLowerCase() === 'ready-for-agent')
    return {
      num: star.number,
      slug: String(star.number),
      title: star.title,
      type: 'issue',
      status: star.status,
      blockedBy: [...star.blocked_by],
      parentIssue: star.parent_issue,
      frontier: star.status === 'frontier',
      readyForAgent: star.ready_for_agent ?? legacyReadyForAgent,
      blocked: star.blocked ?? star.status === 'blocked',
      assignedToViewer:
        viewerLogin !== undefined &&
        star.assignees.some((assignee) => assignee.toLowerCase() === viewerLogin),
    }
  })
}

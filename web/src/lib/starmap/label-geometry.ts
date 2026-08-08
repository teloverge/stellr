import type { Point } from './layout'

export interface LabelBox {
  x0: number
  y0: number
  x1: number
  y1: number
}

export type LabelAlign = 'left' | 'right' | 'center'

export interface OutwardLabelInput {
  parent: Point
  child: Point
  textWidth: number
  fontSize: number
  starRadius: number
  unitScale?: number
}

export interface LabelGeometry {
  x: number
  y: number
  align: LabelAlign
  box: LabelBox
}

export interface ScaledOutwardLabelInput {
  parent: { x: number; y: number }
  child: { x: number; y: number }
  number: number
  title: string
  scale: number
  starRadius: number
}

const DEFAULT_GAP = 8
const VERTICAL_ALIGNMENT_BAND = 0.5
export const SUBISSUE_TITLE_BUDGET = 40
export const ORBIT_LABEL_REFERENCE_SCALE = 0.42
const MAX_SUBISSUE_MARKER = subissueMarker(true, true)
const AVERAGE_GLYPH_EM = 0.56
const HORIZONTAL_PADDING = 3
const VERTICAL_PADDING = 2

export function clipTitle(title: string, budget: number): string {
  if (title.length <= budget) return title
  const cut = title.slice(0, budget)
  const space = cut.lastIndexOf(' ')
  return (space >= budget - 6 && space > 0 ? cut.slice(0, space) : cut.trimEnd()) + '…'
}

export function issueNumberText(number: number): string {
  return `${number < 10 ? '0' : ''}${number}`
}

export function subissueMarker(current: boolean, ready: boolean): string {
  if (current && ready) return 'CURRENT / READY · '
  if (current) return 'CURRENT · '
  return ready ? 'READY · ' : ''
}

export function subissueLabelText(number: number, title: string): string {
  return `${issueNumberText(number)}  ${clipTitle(title, SUBISSUE_TITLE_BUDGET)}`
}

export function estimateLabelWidth(number: number, title: string, fontSize = 14): number {
  const text = `${MAX_SUBISSUE_MARKER}${subissueLabelText(number, title)}`
  return text.length * fontSize * AVERAGE_GLYPH_EM
}

export function orbitLabelFontSize(scale: number): number {
  if (scale < ORBIT_LABEL_REFERENCE_SCALE) {
    return orbitLabelFontSize(ORBIT_LABEL_REFERENCE_SCALE) *
      (scale / ORBIT_LABEL_REFERENCE_SCALE)
  }
  return Math.min(16, Math.max(10, 13 * Math.pow(scale, 0.3)))
}

export function boxesOverlap(left: LabelBox, right: LabelBox): boolean {
  return left.x0 < right.x1 && right.x0 < left.x1 && left.y0 < right.y1 && right.y0 < left.y1
}

export function segmentIntersectsBox(
  start: { x: number; y: number },
  end: { x: number; y: number },
  box: LabelBox,
): boolean {
  const dx = end.x - start.x
  const dy = end.y - start.y
  let entry = 0
  let exit = 1
  const boundaries = [
    { direction: -dx, distance: start.x - box.x0 },
    { direction: dx, distance: box.x1 - start.x },
    { direction: -dy, distance: start.y - box.y0 },
    { direction: dy, distance: box.y1 - start.y },
  ]
  for (const { direction, distance } of boundaries) {
    if (direction === 0) {
      if (distance < 0) return false
      continue
    }
    const ratio = distance / direction
    if (direction < 0) entry = Math.max(entry, ratio)
    else exit = Math.min(exit, ratio)
    if (entry > exit) return false
  }
  return true
}

export function labelBox(
  x: number,
  y: number,
  align: LabelAlign,
  textWidth: number,
  fontSize: number,
  paddingScale = 1,
): LabelBox {
  const textLeft = align === 'left' ? x : align === 'right' ? x - textWidth : x - textWidth / 2
  return {
    x0: textLeft - HORIZONTAL_PADDING * paddingScale,
    y0: y - fontSize * 0.82 - VERTICAL_PADDING * paddingScale,
    x1: textLeft + textWidth + HORIZONTAL_PADDING * paddingScale,
    y1: y + fontSize * 0.22 + VERTICAL_PADDING * paddingScale,
  }
}

export function outwardLabelGeometry(input: OutwardLabelInput): LabelGeometry {
  const dx = input.child.x - input.parent.x
  const dy = input.child.y - input.parent.y
  const length = Math.hypot(dx, dy) || 1
  const ux = dx / length
  const uy = dy / length
  const unitScale = input.unitScale ?? 1
  const distance = input.starRadius + DEFAULT_GAP * unitScale
  const anchorX = input.child.x + ux * distance
  const anchorY = input.child.y + uy * distance
  const vertical = Math.abs(ux) <= Math.abs(uy) * VERTICAL_ALIGNMENT_BAND

  let align: LabelAlign
  let x = anchorX
  let y: number
  if (vertical) {
    align = 'center'
    y = uy < 0
      ? anchorY - input.fontSize * 0.22 - VERTICAL_PADDING * unitScale
      : anchorY + input.fontSize * 0.82 + VERTICAL_PADDING * unitScale
  } else {
    align = ux < 0 ? 'right' : 'left'
    y = anchorY + input.fontSize * 0.3
  }

  const box = labelBox(x, y, align, input.textWidth, input.fontSize, unitScale)
  return { x, y, align, box }
}

export function outwardLabelGeometryAtScale(input: ScaledOutwardLabelInput): LabelGeometry {
  const scale = Math.max(input.scale, Number.EPSILON)
  const fontSize = orbitLabelFontSize(scale)
  const unitScale = Math.min(1, scale / ORBIT_LABEL_REFERENCE_SCALE)
  const screen = outwardLabelGeometry({
    parent: { x: input.parent.x * scale, y: input.parent.y * scale },
    child: { x: input.child.x * scale, y: input.child.y * scale },
    textWidth: estimateLabelWidth(input.number, input.title, fontSize),
    fontSize,
    starRadius: input.starRadius * scale,
    unitScale,
  })
  return {
    x: screen.x / scale,
    y: screen.y / scale,
    align: screen.align,
    box: {
      x0: screen.box.x0 / scale,
      y0: screen.box.y0 / scale,
      x1: screen.box.x1 / scale,
      y1: screen.box.y1 / scale,
    },
  }
}

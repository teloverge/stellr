import { describe, expect, it } from 'vitest'
import {
  estimateLabelWidth,
  orbitLabelFontSize,
  ORBIT_LABEL_REFERENCE_SCALE,
  outwardLabelGeometry,
  outwardLabelGeometryAtScale,
  segmentIntersectsBox,
} from './label-geometry'

const parent = { x: 0, y: 0 }
const base = {
  parent,
  textWidth: 84,
  fontSize: 14,
  starRadius: 10,
}

describe('outward subissue label geometry', () => {
  it('extends horizontal labels away from the parent', () => {
    const right = outwardLabelGeometry({ ...base, child: { x: 100, y: 0 } })
    const left = outwardLabelGeometry({ ...base, child: { x: -100, y: 0 } })

    expect(right.align).toBe('left')
    expect(right.box.x0).toBeGreaterThan(100)
    expect(left.align).toBe('right')
    expect(left.box.x1).toBeLessThan(-100)
  })

  it('centres vertical labels beyond the child orbit', () => {
    const above = outwardLabelGeometry({ ...base, child: { x: 0, y: -100 } })
    const below = outwardLabelGeometry({ ...base, child: { x: 0, y: 100 } })

    expect(above.align).toBe('center')
    expect(above.box.y1).toBeLessThan(-100)
    expect(below.align).toBe('center')
    expect(below.box.y0).toBeGreaterThan(100)
  })

  it('uses a deterministic truncated-title footprint', () => {
    expect(estimateLabelWidth(7, 'Short')).toBe(estimateLabelWidth(7, 'Short'))
    expect(estimateLabelWidth(7, 'A considerably longer subissue title')).toBeGreaterThan(
      estimateLabelWidth(7, 'Short'),
    )
    expect(
      estimateLabelWidth(7, 'A'.repeat(200)),
    ).toBeLessThan(estimateLabelWidth(7, 'A'.repeat(200), 28))
  })

  it('detects relationship segments that cross a reserved label box', () => {
    const box = { x0: 10, y0: 10, x1: 30, y1: 20 }

    expect(segmentIntersectsBox({ x: 0, y: 15 }, { x: 40, y: 15 }, box)).toBe(true)
    expect(segmentIntersectsBox({ x: 15, y: 12 }, { x: 25, y: 18 }, box)).toBe(true)
    expect(segmentIntersectsBox({ x: 0, y: 0 }, { x: 40, y: 5 }, box)).toBe(false)
  })

  it('projects the layout reference bounds to the renderer bounds exactly', () => {
    const number = 31
    const title = 'A descriptive subissue title'
    const world = outwardLabelGeometryAtScale({
      parent,
      child: { x: 180, y: 0 },
      number,
      title,
      scale: ORBIT_LABEL_REFERENCE_SCALE,
      starRadius: 14,
    })
    const scale = ORBIT_LABEL_REFERENCE_SCALE
    const screen = outwardLabelGeometry({
      parent,
      child: { x: 180 * scale, y: 0 },
      textWidth: estimateLabelWidth(number, title, orbitLabelFontSize(scale)),
      fontSize: orbitLabelFontSize(scale),
      starRadius: 14 * scale,
    })

    expect(world.box.x0 * scale).toBeCloseTo(screen.box.x0, 6)
    expect(world.box.y0 * scale).toBeCloseTo(screen.box.y0, 6)
    expect(world.box.x1 * scale).toBeCloseTo(screen.box.x1, 6)
    expect(world.box.y1 * scale).toBeCloseTo(screen.box.y1, 6)

    for (const reducedScale of [0.3, 0.12]) {
      const reduced = outwardLabelGeometryAtScale({
        parent,
        child: { x: 180, y: 0 },
        number,
        title,
        scale: reducedScale,
        starRadius: 14,
      })
      expect(reduced.box.x0).toBeCloseTo(world.box.x0, 6)
      expect(reduced.box.y0).toBeCloseTo(world.box.y0, 6)
      expect(reduced.box.x1).toBeCloseTo(world.box.x1, 6)
      expect(reduced.box.y1).toBeCloseTo(world.box.y1, 6)
    }
  })
})

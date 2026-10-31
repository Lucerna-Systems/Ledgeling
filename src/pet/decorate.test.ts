import { describe, expect, it } from 'vitest'
import { BAND, fractionAlong, inBand, thumbScale } from './decorate'

const edit = { edgeX: 100, edgeY: 400, edgeWidth: 600 }

describe('decorate geometry', () => {
  it('maps a pointer to a fraction along the edge, clamped to its ends', () => {
    expect(fractionAlong(250, edit)).toBe(0.25)
    expect(fractionAlong(0, edit)).toBe(0)
    expect(fractionAlong(5000, edit)).toBe(1)
    expect(fractionAlong(10, { edgeX: 0, edgeWidth: 0 })).toBe(0.5)
  })

  it('accepts drops just above the edge and refuses the rest of the screen', () => {
    expect(inBand(300, 390, edit)).toBe(true)
    expect(inBand(300, 400 - BAND, edit)).toBe(true)
    expect(inBand(300, 400 - BAND - 1, edit)).toBe(false)
    expect(inBand(50, 390, edit)).toBe(false)
    expect(inBand(300, 600, edit)).toBe(false)
  })

  it('fits a thumbnail inside its box without blowing small props up', () => {
    expect(thumbScale(48, 64)).toBeCloseTo(36 / 64)
    expect(thumbScale(12, 12)).toBe(2)
  })
})

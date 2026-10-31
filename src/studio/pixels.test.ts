import { describe, expect, it } from 'vitest'
import { FRAME, blankFrame } from './model'
import { applyGradient, drop, ellipsePoints, erase, flipPixels, lift, linePoints, magicWand, maskBounds, nudge, opaqueMask, rectMask, rectPoints, shiftMask, thicken } from './pixels'

const PET = { width: FRAME, height: FRAME }

const put = (f: Uint8ClampedArray, x: number, y: number, c: number[]) => f.set(c, (y * FRAME + x) * 4)
const at = (f: Uint8ClampedArray, x: number, y: number) => Array.from(f.subarray((y * FRAME + x) * 4, (y * FRAME + x) * 4 + 4))
const count = (m: Uint8Array) => m.reduce((a, b) => a + b, 0)

describe('magic wand', () => {
  it('selects only the connected region of the colour under it', () => {
    const f = blankFrame()
    for (const x of [2, 3]) put(f, x, 2, [255, 0, 0, 255])
    put(f, 10, 2, [255, 0, 0, 255])
    const mask = magicWand(PET, f, 2, 2, 0, true)
    expect(count(mask)).toBe(2)
    expect(count(magicWand(PET, f, 2, 2, 0, false))).toBe(3)
  })
  it('treats all transparent pixels as one colour and honours tolerance', () => {
    const f = blankFrame()
    put(f, 0, 0, [100, 0, 0, 255]); put(f, 1, 0, [110, 0, 0, 255])
    expect(count(magicWand(PET, f, 0, 0, 0, true))).toBe(1)
    expect(count(magicWand(PET, f, 0, 0, 10, true))).toBe(2)
    expect(count(magicWand(PET, f, 20, 20, 0, true))).toBe(FRAME * FRAME - 2)
  })
})

describe('shapes', () => {
  it('draws a connected line including both ends', () => {
    const line = linePoints(0, 0, 5, 2)
    expect(line[0]).toEqual([0, 0]); expect(line.at(-1)).toEqual([5, 2]); expect(line).toHaveLength(6)
  })
  it('draws rectangle outlines and fills', () => {
    expect(rectPoints(1, 1, 4, 3, false)).toHaveLength(10)
    expect(rectPoints(1, 1, 4, 3, true)).toHaveLength(12)
  })
  it('draws an ellipse whose outline is inside its fill', () => {
    const filled = new Set(ellipsePoints(2, 2, 12, 8, true).map(p => p.join()))
    const outline = ellipsePoints(2, 2, 12, 8, false)
    expect(outline.length).toBeLessThan(filled.size)
    expect(outline.every(p => filled.has(p.join()))).toBe(true)
  })
  it('thickens without duplicating cells', () => {
    expect(thicken([[5, 5]], 3)).toHaveLength(9)
    expect(thicken([[5, 5], [6, 5]], 2)).toHaveLength(6)
  })
})

describe('gradients', () => {
  const opts = { from: [0, 0] as [number, number], to: [10, 0] as [number, number], start: [0, 0, 0, 255] as [number, number, number, number], end: [200, 100, 50, 255] as [number, number, number, number], kind: 'linear' as const, dither: false }
  it('runs from the start colour to the end colour and stays inside the mask', () => {
    const f = blankFrame()
    applyGradient(PET, f, rectMask(PET, 0, 0, 10, 0), opts)
    expect(at(f, 0, 0)).toEqual([0, 0, 0, 255])
    expect(at(f, 5, 0)).toEqual([100, 50, 25, 255])
    expect(at(f, 10, 0)).toEqual([200, 100, 50, 255])
    expect(at(f, 5, 1)).toEqual([0, 0, 0, 0])
  })
  it('can fade to transparent', () => {
    const f = blankFrame()
    applyGradient(PET, f, null, { ...opts, end: null })
    expect(at(f, 10, 3)[3]).toBe(0)
    expect(at(f, 0, 3)[3]).toBe(255)
  })
  it('dithers using only the two end colours', () => {
    const f = blankFrame()
    applyGradient(PET, f, rectMask(PET, 0, 0, 10, 3), { ...opts, dither: true })
    const reds = new Set<number>()
    for (let y = 0; y < 4; y++) for (let x = 0; x <= 10; x++) reds.add(at(f, x, y)[0]!)
    expect([...reds].sort((a, b) => a - b)).toEqual([0, 200])
  })
  it('radiates from the centre', () => {
    const f = blankFrame()
    applyGradient(PET, f, null, { ...opts, from: [20, 20], to: [30, 20], kind: 'radial' })
    expect(at(f, 20, 20)).toEqual([0, 0, 0, 255])
    expect(at(f, 20, 40)).toEqual([200, 100, 50, 255])
  })
})

describe('transforms', () => {
  it('flips the whole frame', () => {
    const f = blankFrame(); put(f, 0, 0, [1, 2, 3, 255])
    flipPixels(PET, f, 'horizontal', null)
    expect(at(f, FRAME - 1, 0)).toEqual([1, 2, 3, 255]); expect(at(f, 0, 0)[3]).toBe(0)
  })
  it('flips within a selection only', () => {
    const f = blankFrame(); put(f, 2, 2, [9, 9, 9, 255])
    flipPixels(PET, f, 'horizontal', rectMask(PET, 2, 2, 5, 2))
    expect(at(f, 5, 2)).toEqual([9, 9, 9, 255]); expect(at(f, 2, 2)[3]).toBe(0)
  })
  it('nudges everything or just the selection', () => {
    const f = blankFrame(); put(f, 1, 1, [1, 1, 1, 255]); put(f, 8, 8, [2, 2, 2, 255])
    nudge(PET, f, 1, 0, rectMask(PET, 0, 0, 3, 3))
    expect(at(f, 2, 1)).toEqual([1, 1, 1, 255]); expect(at(f, 8, 8)).toEqual([2, 2, 2, 255])
    nudge(PET, f, 0, 1, null)
    expect(at(f, 2, 2)[3]).toBe(255); expect(at(f, 8, 9)[3]).toBe(255)
  })
  it('loses pixels pushed off the edge', () => {
    const f = blankFrame(); put(f, FRAME - 1, 0, [1, 1, 1, 255])
    nudge(PET, f, 1, 0, null)
    expect(f.some(v => v)).toBe(false)
  })
  it('lifts, erases and drops a selection to move it', () => {
    const f = blankFrame(); put(f, 3, 3, [7, 7, 7, 255])
    const mask = rectMask(PET, 3, 3, 3, 3)
    const layer = lift(f, mask)
    erase(f, mask)
    expect(f.some(v => v)).toBe(false)
    drop(PET, f, layer, 4, 1)
    expect(at(f, 7, 4)).toEqual([7, 7, 7, 255])
    expect(maskBounds(PET, shiftMask(PET, mask, 4, 1))).toEqual({ x0: 7, y0: 4, x1: 7, y1: 4 })
    expect(count(opaqueMask(layer))).toBe(1)
  })
})

describe('any size', () => {
  // A tall, narrow decor prop: 5 wide, 9 high.
  const TALL = { width: 5, height: 9 }
  const image = () => new Uint8ClampedArray(TALL.width * TALL.height * 4)
  const set = (f: Uint8ClampedArray, x: number, y: number, c: number[]) => f.set(c, (y * TALL.width + x) * 4)
  const get = (f: Uint8ClampedArray, x: number, y: number) => Array.from(f.subarray((y * TALL.width + x) * 4, (y * TALL.width + x) * 4 + 4))

  it('selects, fills and bounds within the image', () => {
    expect(count(rectMask(TALL, -3, -3, 20, 20))).toBe(45)
    const f = image(); set(f, 4, 8, [1, 2, 3, 255])
    expect(count(magicWand(TALL, f, 0, 0, 0, true))).toBe(44)
    expect(maskBounds(TALL, magicWand(TALL, f, 4, 8, 0, false))).toEqual({ x0: 4, y0: 8, x1: 4, y1: 8 })
    applyGradient(TALL, f, null, { from: [0, 0], to: [0, 8], start: [0, 0, 0, 255], end: [80, 0, 0, 255], kind: 'linear', dither: false })
    expect(get(f, 4, 8)).toEqual([80, 0, 0, 255])
  })
  it('flips, nudges and drops along the right rows', () => {
    const f = image(); set(f, 0, 7, [5, 5, 5, 255])
    flipPixels(TALL, f, 'horizontal', null)
    expect(get(f, 4, 7)).toEqual([5, 5, 5, 255])
    flipPixels(TALL, f, 'vertical', null)
    expect(get(f, 4, 1)).toEqual([5, 5, 5, 255])
    nudge(TALL, f, 0, 7, null)
    expect(get(f, 4, 8)).toEqual([5, 5, 5, 255])
    nudge(TALL, f, 1, 0, null)
    expect(f.some(v => v)).toBe(false)
    const layer = image(); set(layer, 0, 0, [9, 9, 9, 255])
    drop(TALL, f, layer, 2, 6)
    expect(get(f, 2, 6)).toEqual([9, 9, 9, 255])
    expect(maskBounds(TALL, shiftMask(TALL, opaqueMask(layer), 4, 8))).toEqual({ x0: 4, y0: 8, x1: 4, y1: 8 })
  })
})

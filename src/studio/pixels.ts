import { FRAME } from './model'

/** One byte per pixel: 1 where selected. Same layout as a frame, a quarter the size. */
export type Mask = Uint8Array
export type Rgba = [number, number, number, number]
export type Point = [number, number]

export const emptyMask = (): Mask => new Uint8Array(FRAME * FRAME)
export const inside = (x: number, y: number) => x >= 0 && y >= 0 && x < FRAME && y < FRAME
export const maskHas = (mask: Mask, x: number, y: number) => inside(x, y) && mask[y * FRAME + x] === 1
export const maskEmpty = (mask: Mask) => !mask.some(Boolean)

export function rectMask(x0: number, y0: number, x1: number, y1: number): Mask {
  const mask = emptyMask()
  for (let y = Math.max(0, Math.min(y0, y1)); y <= Math.min(FRAME - 1, Math.max(y0, y1)); y++)
    for (let x = Math.max(0, Math.min(x0, x1)); x <= Math.min(FRAME - 1, Math.max(x0, x1)); x++) mask[y * FRAME + x] = 1
  return mask
}

export function invertMask(mask: Mask): Mask {
  return mask.map(v => (v ? 0 : 1)) as Mask
}

export function combineMasks(a: Mask, b: Mask, mode: 'add' | 'subtract'): Mask {
  return a.map((v, i) => (mode === 'add' ? (v || b[i] ? 1 : 0) : (v && !b[i] ? 1 : 0))) as Mask
}

export function shiftMask(mask: Mask, dx: number, dy: number): Mask {
  const out = emptyMask()
  for (let y = 0; y < FRAME; y++) for (let x = 0; x < FRAME; x++)
    if (mask[y * FRAME + x] && inside(x + dx, y + dy)) out[(y + dy) * FRAME + x + dx] = 1
  return out
}

function close(pixels: Uint8ClampedArray, i: number, target: ArrayLike<number>, tolerance: number) {
  // Fully transparent pixels are all the same colour, whatever their stored RGB.
  if (pixels[i + 3] === 0 && target[3] === 0) return true
  for (let k = 0; k < 4; k++) if (Math.abs(pixels[i + k]! - target[k]!) > tolerance) return false
  return true
}

/** The magic wand: pixels near the colour under (x, y), either the connected
 *  region or everywhere in the frame. Tolerance is the largest allowed
 *  per-channel difference, 0 to 255. */
export function magicWand(pixels: Uint8ClampedArray, x: number, y: number, tolerance: number, contiguous: boolean): Mask {
  const mask = emptyMask()
  if (!inside(x, y)) return mask
  const target = Array.from(pixels.subarray((y * FRAME + x) * 4, (y * FRAME + x) * 4 + 4))
  if (!contiguous) {
    for (let p = 0; p < FRAME * FRAME; p++) if (close(pixels, p * 4, target, tolerance)) mask[p] = 1
    return mask
  }
  const queue: number[] = [y * FRAME + x]
  while (queue.length) {
    const p = queue.pop()!
    if (mask[p] || !close(pixels, p * 4, target, tolerance)) continue
    mask[p] = 1
    const px = p % FRAME, py = Math.floor(p / FRAME)
    if (px > 0) queue.push(p - 1)
    if (px < FRAME - 1) queue.push(p + 1)
    if (py > 0) queue.push(p - FRAME)
    if (py < FRAME - 1) queue.push(p + FRAME)
  }
  return mask
}

// ---- shapes: each returns the cells to paint ----

export function linePoints(x0: number, y0: number, x1: number, y1: number): Point[] {
  const out: Point[] = []
  const dx = Math.abs(x1 - x0), dy = -Math.abs(y1 - y0), sx = x0 < x1 ? 1 : -1, sy = y0 < y1 ? 1 : -1
  let err = dx + dy
  for (;;) {
    out.push([x0, y0])
    if (x0 === x1 && y0 === y1) return out
    const e2 = 2 * err
    if (e2 >= dy) { err += dy; x0 += sx }
    if (e2 <= dx) { err += dx; y0 += sy }
  }
}

export function rectPoints(x0: number, y0: number, x1: number, y1: number, filled: boolean): Point[] {
  const [l, r, t, b] = [Math.min(x0, x1), Math.max(x0, x1), Math.min(y0, y1), Math.max(y0, y1)]
  const out: Point[] = []
  for (let y = t; y <= b; y++) for (let x = l; x <= r; x++) if (filled || x === l || x === r || y === t || y === b) out.push([x, y])
  return out
}

/** An ellipse inscribed in the dragged box. */
export function ellipsePoints(x0: number, y0: number, x1: number, y1: number, filled: boolean): Point[] {
  const [l, r, t, b] = [Math.min(x0, x1), Math.max(x0, x1), Math.min(y0, y1), Math.max(y0, y1)]
  const cx = (l + r) / 2, cy = (t + b) / 2
  const rx = (r - l) / 2 + 0.5, ry = (b - t) / 2 + 0.5
  const inEllipse = (x: number, y: number) => ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1
  const out: Point[] = []
  for (let y = t; y <= b; y++) for (let x = l; x <= r; x++) {
    if (!inEllipse(x, y)) continue
    // The outline is the filled shape's cells that touch the outside.
    if (filled || !inEllipse(x - 1, y) || !inEllipse(x + 1, y) || !inEllipse(x, y - 1) || !inEllipse(x, y + 1)) out.push([x, y])
  }
  return out
}

/** Grows each cell into a square brush. Sizes above 1 are centred as well as an even width allows. */
export function thicken(points: Point[], size: number): Point[] {
  if (size <= 1) return points
  const seen = new Set<number>()
  const out: Point[] = []
  const lo = -Math.floor((size - 1) / 2)
  for (const [x, y] of points) for (let j = 0; j < size; j++) for (let i = 0; i < size; i++) {
    const px = x + lo + i, py = y + lo + j
    const key = (py + 64) * 4096 + px + 64
    if (!seen.has(key)) { seen.add(key); out.push([px, py]) }
  }
  return out
}

// ---- gradients ----

export interface GradientOptions {
  from: Point
  to: Point
  start: Rgba
  /** `null` fades to transparent instead of to a second colour. */
  end: Rgba | null
  kind: 'linear' | 'radial'
  /** Ordered dithering: only two tones, the way pixel art usually does it, with no in-between colours. */
  dither: boolean
}

const BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]

/** Paints a gradient across the masked cells. With no mask it covers the whole frame. */
export function applyGradient(pixels: Uint8ClampedArray, mask: Mask | null, o: GradientOptions) {
  const [fx, fy] = o.from
  const dx = o.to[0] - fx, dy = o.to[1] - fy
  const length2 = dx * dx + dy * dy
  const radius = Math.hypot(dx, dy)
  for (let y = 0; y < FRAME; y++) for (let x = 0; x < FRAME; x++) {
    if (mask && !mask[y * FRAME + x]) continue
    let t: number
    if (o.kind === 'radial') t = radius === 0 ? 0 : Math.hypot(x - fx, y - fy) / radius
    else t = length2 === 0 ? 0 : ((x - fx) * dx + (y - fy) * dy) / length2
    t = Math.min(1, Math.max(0, t))
    if (o.dither) t = t > (BAYER[y % 4]![x % 4]! + 0.5) / 16 ? 1 : 0
    const end = o.end ?? [o.start[0], o.start[1], o.start[2], 0]
    const i = (y * FRAME + x) * 4
    for (let k = 0; k < 4; k++) pixels[i + k] = Math.round(o.start[k]! + (end[k]! - o.start[k]!) * t)
  }
}

// ---- transforms ----

export function flipPixels(pixels: Uint8ClampedArray, axis: 'horizontal' | 'vertical', mask: Mask | null) {
  const copy = new Uint8ClampedArray(pixels)
  const bounds = mask ? maskBounds(mask) : { x0: 0, y0: 0, x1: FRAME - 1, y1: FRAME - 1 }
  if (!bounds) return
  for (let y = bounds.y0; y <= bounds.y1; y++) for (let x = bounds.x0; x <= bounds.x1; x++) {
    if (mask && !mask[y * FRAME + x]) continue
    const sx = axis === 'horizontal' ? bounds.x0 + bounds.x1 - x : x
    const sy = axis === 'vertical' ? bounds.y0 + bounds.y1 - y : y
    // Selected pixels swap only within the selection, so a lopsided mask clips instead of smearing.
    if (mask && !mask[sy * FRAME + sx]) continue
    pixels.set(copy.subarray((sy * FRAME + sx) * 4, (sy * FRAME + sx) * 4 + 4), (y * FRAME + x) * 4)
  }
}

export function maskBounds(mask: Mask) {
  let x0 = FRAME, y0 = FRAME, x1 = -1, y1 = -1
  for (let p = 0; p < mask.length; p++) {
    if (!mask[p]) continue
    const x = p % FRAME, y = Math.floor(p / FRAME)
    x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y)
  }
  return x1 < 0 ? null : { x0, y0, x1, y1 }
}

/** Moves pixels by (dx, dy); anything pushed off the edge is lost. With a mask only the selected pixels move. */
export function nudge(pixels: Uint8ClampedArray, dx: number, dy: number, mask: Mask | null) {
  const copy = new Uint8ClampedArray(pixels)
  if (mask) erase(pixels, mask)
  else pixels.fill(0)
  for (let y = 0; y < FRAME; y++) for (let x = 0; x < FRAME; x++) {
    if (mask && !mask[y * FRAME + x]) continue
    if (!inside(x + dx, y + dy)) continue
    pixels.set(copy.subarray((y * FRAME + x) * 4, (y * FRAME + x) * 4 + 4), ((y + dy) * FRAME + x + dx) * 4)
  }
}

/** A copy of the selected pixels, everything else transparent. */
export function lift(pixels: Uint8ClampedArray, mask: Mask): Uint8ClampedArray {
  const out = new Uint8ClampedArray(pixels.length)
  for (let p = 0; p < mask.length; p++) if (mask[p]) out.set(pixels.subarray(p * 4, p * 4 + 4), p * 4)
  return out
}

/** Clears the selected pixels. */
export function erase(pixels: Uint8ClampedArray, mask: Mask) {
  for (let p = 0; p < mask.length; p++) if (mask[p]) pixels.fill(0, p * 4, p * 4 + 4)
}

/** Lays a floating layer over the frame, shifted by (dx, dy). Transparent layer pixels leave what is beneath. */
export function drop(pixels: Uint8ClampedArray, layer: Uint8ClampedArray, dx: number, dy: number) {
  for (let y = 0; y < FRAME; y++) for (let x = 0; x < FRAME; x++) {
    const i = (y * FRAME + x) * 4
    if (!layer[i + 3] || !inside(x + dx, y + dy)) continue
    pixels.set(layer.subarray(i, i + 4), ((y + dy) * FRAME + x + dx) * 4)
  }
}

/** The cells where the layer has something to show, as a selection. */
export function opaqueMask(layer: Uint8ClampedArray): Mask {
  const mask = emptyMask()
  for (let p = 0; p < mask.length; p++) if (layer[p * 4 + 3]) mask[p] = 1
  return mask
}

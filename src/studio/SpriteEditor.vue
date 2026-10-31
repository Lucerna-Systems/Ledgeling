<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, useSlots, watch } from 'vue'
import { ArrowLeft, ArrowLeftRight, ArrowRight, Blend, BoxSelect, Circle, ClipboardPaste, Copy, Eraser, FlipHorizontal2, FlipVertical2, FoldHorizontal, Grid3x3, Layers, Minus, PaintBucket, Pause, Pencil, Pipette, Play, Plus, Redo2, Scissors, Square, Trash2, Undo2, WandSparkles, ZoomIn, ZoomOut } from '@lucide/vue'
import { MAX_FRAMES } from './model'
import { applyGradient, combineMasks, drop, ellipsePoints, emptyMask, erase, flipPixels, invertMask, lift, linePoints, magicWand, maskEmpty, maskHas, nudge, opaqueMask, rectMask, rectPoints, shiftMask, thicken, type Mask, type Point, type Rgba } from './pixels'
import { thumbnailer } from './thumbs'
import './studio.css'

/** The Studio's pixel editor, for any `width × height`. It edits `frames` in
 *  place: a pet clip's frames with `animation` on, or a decor prop's one image
 *  with it off. What the frames belong to (clips, props) is the host's business,
 *  shown through the `list`, `head`, `empty` and `overlay` slots. */
const props = withDefaults(defineProps<{
  width: number
  height: number
  /** Edited in place. Empty shows the `empty` slot instead of the canvas. */
  frames: Uint8ClampedArray[]
  /** Frame strip, onion skin and a playing preview. Off, it is one still image. */
  animation?: boolean
  maxFrames?: number
  fps?: number
  loop?: boolean
}>(), { animation: false, maxFrames: MAX_FRAMES, fps: 8, loop: true })
/** Pixel edits are invisible to Vue (typed arrays are not reactive), so the
 *  host hears about them here to re-check what still needs drawing. `reveal`
 *  asks it to show the frames holding an image that undo just changed, which
 *  may belong to another clip or prop. */
const emit = defineEmits<{ edited: []; reveal: [pixels: Uint8ClampedArray]; 'update:fps': [fps: number]; 'update:loop': [loop: boolean] }>()
const slots = useSlots()
/** Bumped after every pixel change; thumbnails and the preview redraw from it. */
const version = ref(0)
watch(version, () => emit('edited'))

const size = computed(() => ({ width: props.width, height: props.height }))
const frameIndex = ref(0)
const frame = computed(() => props.frames[frameIndex.value])
const blankImage = () => new Uint8ClampedArray(props.width * props.height * 4)
const insideImage = (x: number, y: number) => x >= 0 && y >= 0 && x < props.width && y < props.height

// ---- zoom and pan ----
const MAX_ZOOM = 16
const stage = ref<HTMLElement | null>(null)
const stageSize = ref({ w: 480, h: 480 })
/** `x` and `y` are where the canvas's top-left sits in the stage; they only matter while zoomed in. */
const view = ref({ zoom: 1, x: 0, y: 0 })
const spaceDown = ref(false)
const panning = ref(false)
/** Screen pixels per image pixel at 100%: the image fitted to the stage, never under 64 px across. */
const fit = computed(() => Math.max(64 / Math.max(props.width, props.height), Math.min(stageSize.value.w / props.width, stageSize.value.h / props.height)))
const shownSize = (zoom = view.value.zoom) => ({ w: props.width * fit.value * zoom, h: props.height * fit.value * zoom })
/** Keeps a zoomed canvas covering the stage, and a fitted one centred. */
const clampAxis = (pos: number, size: number, dim: number) => size <= dim ? (dim - size) / 2 : Math.min(0, Math.max(dim - size, pos))
function shown() {
  const { w, h } = shownSize()
  return { x: clampAxis(view.value.x, w, stageSize.value.w), y: clampAxis(view.value.y, h, stageSize.value.h) }
}
const canvasStyle = computed(() => {
  const { w, h } = shownSize()
  const at = shown()
  return { width: `${w}px`, height: `${h}px`, left: `${at.x}px`, top: `${at.y}px` }
})
/** Zooms keeping the point under (cx, cy), in stage pixels, where it is. */
function zoomAt(factor: number, cx: number, cy: number) {
  const zoom = Math.min(MAX_ZOOM, Math.max(1, view.value.zoom * factor))
  const { w, h } = shownSize()
  const at = shown()
  const [fx, fy] = [(cx - at.x) / w, (cy - at.y) / h]
  const next = shownSize(zoom)
  view.value = { zoom, x: cx - fx * next.w, y: cy - fy * next.h }
}
const zoomBy = (factor: number) => zoomAt(factor, stageSize.value.w / 2, stageSize.value.h / 2)
const fitView = () => { view.value = { zoom: 1, x: 0, y: 0 } }
function stagePoint(event: { clientX: number; clientY: number }): Point {
  const box = stage.value!.getBoundingClientRect()
  return [event.clientX - box.left, event.clientY - box.top]
}
/** Scrolling zooms; so does a trackpad pinch, which browsers report as ctrl + wheel. */
function wheel(event: WheelEvent) {
  event.preventDefault()
  const lines = event.deltaMode === 1 ? 16 : 1
  const delta = Math.max(-120, Math.min(120, event.deltaY * lines))
  const [cx, cy] = stagePoint(event)
  zoomAt(Math.exp(-delta * (event.ctrlKey ? 0.012 : 0.003)), cx, cy)
}
// WebKit (the macOS app) reports a pinch as gesture events instead of ctrl + wheel.
let gestureStart = 1
function gestureBegin(event: Event) { event.preventDefault(); gestureStart = view.value.zoom }
function gestureChange(event: Event) {
  event.preventDefault()
  const { scale, clientX, clientY } = event as Event & { scale: number; clientX: number; clientY: number }
  const [cx, cy] = stagePoint({ clientX, clientY })
  zoomAt(gestureStart * scale / view.value.zoom, cx, cy)
}
let observer: ResizeObserver | null = null
watch(stage, (el, old) => {
  observer?.disconnect()
  if (old) {
    old.removeEventListener('wheel', wheel)
    for (const [name, handler] of [['gesturestart', gestureBegin], ['gesturechange', gestureChange]] as const) old.removeEventListener(name, handler)
  }
  if (!el) return
  el.addEventListener('wheel', wheel, { passive: false })
  el.addEventListener('gesturestart', gestureBegin)
  el.addEventListener('gesturechange', gestureChange)
  if (typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(() => { stageSize.value = { w: el.clientWidth, h: el.clientHeight } })
    observer.observe(el)
  }
  stageSize.value = { w: el.clientWidth, h: el.clientHeight }
})

// ---- tools ----
type Tool = 'pencil' | 'eraser' | 'fill' | 'picker' | 'line' | 'rect' | 'ellipse' | 'gradient' | 'select' | 'wand'
const tool = ref<Tool>('pencil')
const tools: { id: Tool; label: string; key: string; icon: unknown; group: number }[] = [
  { id: 'pencil', label: 'Pencil', key: 'b', icon: Pencil, group: 0 },
  { id: 'eraser', label: 'Eraser', key: 'e', icon: Eraser, group: 0 },
  { id: 'fill', label: 'Fill', key: 'g', icon: PaintBucket, group: 0 },
  { id: 'gradient', label: 'Gradient: drag across a shape or selection', key: 'd', icon: Blend, group: 0 },
  { id: 'picker', label: 'Pick a colour', key: 'i', icon: Pipette, group: 0 },
  { id: 'line', label: 'Line', key: 'l', icon: Minus, group: 1 },
  { id: 'rect', label: 'Rectangle', key: 'r', icon: Square, group: 1 },
  { id: 'ellipse', label: 'Ellipse', key: 'o', icon: Circle, group: 1 },
  { id: 'select', label: 'Select a box (drag inside a selection to move it)', key: 's', icon: BoxSelect, group: 2 },
  { id: 'wand', label: 'Magic wand: select similar colours (Shift adds, Alt removes)', key: 'w', icon: WandSparkles, group: 2 },
]
const brush = ref(1)
const filledShapes = ref(false)
const gradientKind = ref<'linear' | 'radial'>('linear')
const gradientEnd = ref<'colour' | 'clear'>('colour')
const dither = ref(false)
const tolerance = ref(0)
const contiguous = ref(true)
const strokeTools: Tool[] = ['pencil', 'eraser', 'line', 'rect', 'ellipse']
const mirror = ref(false)
const grid = ref(true)
const onion = ref(true)
/** Sweetie 16 by GrafxKid: a small, friendly palette that suits pixel pets. */
const palette = ['#1a1c2c', '#5d275d', '#b13e53', '#ef7d57', '#ffcd75', '#a7f070', '#38b764', '#257179', '#29366f', '#3b5dc9', '#41a6f6', '#73eff7', '#f4f4f4', '#94b0c2', '#566c86', '#333c57']
const color = ref('#ef7d57')
/** The second colour: the far end of a gradient. Shift-click a swatch to set it, X swaps the two. */
const color2 = ref('#ffcd75')
const recent = ref<string[]>([])
function rgba(hex: string): Rgba {
  const v = hex.replace('#', '')
  return [parseInt(v.slice(0, 2), 16), parseInt(v.slice(2, 4), 16), parseInt(v.slice(4, 6), 16), 255]
}
const hex = (r: number, g: number, b: number) => `#${[r, g, b].map(n => n.toString(16).padStart(2, '0')).join('')}`
function useColor(value: string, secondary = false) {
  if (secondary) { color2.value = value; return }
  color.value = value
  if (tool.value === 'eraser' || tool.value === 'picker') tool.value = 'pencil'
}
function swapColors() { [color.value, color2.value] = [color2.value, color.value] }

// ---- undo: snapshots of an image before each stroke ----
/** `target` is the image itself, so undo finds it again after the host has
 *  switched to another clip or prop. */
interface Snapshot { target: Uint8ClampedArray; pixels: Uint8ClampedArray }
const undoStack: Snapshot[] = []
const redoStack: Snapshot[] = []
const canUndo = ref(false)
const canRedo = ref(false)
function remember() {
  if (!frame.value) return
  undoStack.push({ target: frame.value, pixels: new Uint8ClampedArray(frame.value) })
  if (undoStack.length > 100) undoStack.shift()
  redoStack.length = 0
  canUndo.value = true; canRedo.value = false
}
function restore(from: Snapshot[], to: Snapshot[]) {
  const snap = from.pop()
  if (!snap) return
  to.push({ ...snap, pixels: new Uint8ClampedArray(snap.target) })
  snap.target.set(snap.pixels)
  canUndo.value = undoStack.length > 0; canRedo.value = redoStack.length > 0
  showImage(snap.target)
  version.value++
}
const undo = () => restore(undoStack, redoStack)
const redo = () => restore(redoStack, undoStack)
/** Brings an image into view: here if it is one of our frames, otherwise the host switches first. */
function showImage(target: Uint8ClampedArray) {
  const index = props.frames.indexOf(target)
  if (index >= 0) { frameIndex.value = index; return }
  emit('reveal', target)
  // After the host has switched, the frames watcher has reset to frame 0; point at the right one.
  void nextTick(() => {
    const found = props.frames.indexOf(target)
    if (found >= 0) frameIndex.value = found
  })
}

// ---- selection ----
/** The selected cells, or null. Replaced rather than edited so the canvas redraws. */
const selection = shallowRef<Mask | null>(null)
const hasSelection = computed(() => !!selection.value)
function select(mask: Mask | null) { selection.value = mask && !maskEmpty(mask) ? mask : null }
function combine(previous: Mask | null, next: Mask, event: { shiftKey: boolean; altKey: boolean }) {
  if (previous && event.shiftKey) return combineMasks(previous, next, 'add')
  if (previous && event.altKey) return combineMasks(previous, next, 'subtract')
  return next
}
const allowed = (x: number, y: number) => !selection.value || maskHas(size.value, selection.value, x, y)
watch(frameIndex, () => { selection.value = null })
/** What copy and cut hold: a full-size layer, transparent outside what was copied. */
const clipboard = shallowRef<Uint8ClampedArray | null>(null)

function selectAll() { select(rectMask(size.value, 0, 0, props.width - 1, props.height - 1)) }
function deselect() { select(null) }
function invertSelection() { select(invertMask(selection.value ?? emptyMask(size.value))) }
/** Runs an edit on the current frame as one undo step. */
function edit(change: (pixels: Uint8ClampedArray) => void) {
  if (!frame.value) return
  remember()
  change(frame.value)
  version.value++
}
function copySelection(cut: boolean) {
  if (!frame.value) return
  clipboard.value = selection.value ? lift(frame.value, selection.value) : new Uint8ClampedArray(frame.value)
  if (cut) deleteSelection()
}
function deleteSelection() { edit(pixels => selection.value ? erase(pixels, selection.value) : pixels.fill(0)) }
function paste() {
  const layer = clipboard.value
  // A layer copied from an image of another size would land scrambled.
  if (!layer || layer.length !== props.width * props.height * 4) return
  edit(pixels => drop(size.value, pixels, layer, 0, 0))
  select(opaqueMask(layer))
  tool.value = 'select'
}
function flip(axis: 'horizontal' | 'vertical') { edit(pixels => flipPixels(size.value, pixels, axis, selection.value)) }
function nudgeBy(dx: number, dy: number) {
  edit(pixels => nudge(size.value, pixels, dx, dy, selection.value))
  if (selection.value) select(shiftMask(size.value, selection.value, dx, dy))
}

// ---- painting ----
function setPixel(pixels: Uint8ClampedArray, x: number, y: number, value: Rgba) {
  if (!insideImage(x, y) || !allowed(x, y)) return
  pixels.set(value, (y * props.width + x) * 4)
}
/** Paints cells with the brush, mirrored when mirror is on. */
function paintCells(points: Point[], value: Rgba) {
  const pixels = frame.value
  if (!pixels) return
  for (const [x, y] of thicken(points, brush.value)) {
    setPixel(pixels, x, y, value)
    if (mirror.value) setPixel(pixels, props.width - 1 - x, y, value)
  }
}
const strokeValue = (): Rgba => tool.value === 'eraser' ? [0, 0, 0, 0] : rgba(color.value)
function paint(x: number, y: number) { paintCells([[x, y]], strokeValue()) }
function fill(x: number, y: number) {
  const pixels = frame.value
  if (!pixels) return
  const at = (px: number, py: number) => (py * props.width + px) * 4
  const target = Array.from(pixels.subarray(at(x, y), at(x, y) + 4))
  const value = rgba(color.value)
  if (target.every((v, i) => v === value[i])) return
  const same = (i: number) => target.every((v, k) => pixels[i + k] === v)
  const queue = [[x, y]]
  while (queue.length) {
    const [px, py] = queue.pop()!
    if (!insideImage(px!, py!) || !same(at(px!, py!)) || !allowed(px!, py!)) continue
    pixels.set(value, at(px!, py!))
    queue.push([px! + 1, py!], [px! - 1, py!], [px!, py! + 1], [px!, py! - 1])
  }
}
function pick(x: number, y: number, secondary = false) {
  const pixels = frame.value
  if (!pixels || !insideImage(x, y)) return
  const i = (y * props.width + x) * 4
  if (pixels[i + 3]) useColor(hex(pixels[i]!, pixels[i + 1]!, pixels[i + 2]!), secondary)
}

const canvas = ref<HTMLCanvasElement | null>(null)
/** What the current drag is doing, if anything. */
type Drag =
  | { kind: 'stroke'; last: Point }
  | { kind: 'shape'; start: Point; base: Uint8ClampedArray }
  | { kind: 'gradient'; start: Point; base: Uint8ClampedArray; region: Mask }
  | { kind: 'box'; start: Point; previous: Mask | null; event: { shiftKey: boolean; altKey: boolean } }
  | { kind: 'move'; start: Point; base: Uint8ClampedArray; layer: Uint8ClampedArray; mask: Mask }
  | { kind: 'pan'; from: Point; x: number; y: number }
let drag: Drag | null = null
let painted = false
function cell(event: PointerEvent): Point {
  const box = canvas.value!.getBoundingClientRect()
  return [Math.floor(((event.clientX - box.left) / box.width) * props.width), Math.floor(((event.clientY - box.top) / box.height) * props.height)]
}
const clampX = (n: number) => Math.min(props.width - 1, Math.max(0, n))
const clampY = (n: number) => Math.min(props.height - 1, Math.max(0, n))
function shapeCells(kind: Tool, [x0, y0]: Point, [x1, y1]: Point): Point[] {
  if (kind === 'line') return linePoints(x0, y0, x1, y1)
  if (kind === 'rect') return rectPoints(x0, y0, x1, y1, filledShapes.value)
  return ellipsePoints(x0, y0, x1, y1, filledShapes.value)
}
function gradientAt(base: Uint8ClampedArray, region: Mask, from: Point, to: Point) {
  const pixels = frame.value!
  pixels.set(base)
  applyGradient(size.value, pixels, region, {
    from, to, start: rgba(color.value), end: gradientEnd.value === 'clear' ? null : rgba(color2.value),
    kind: gradientKind.value, dither: dither.value,
  })
}
function down(event: PointerEvent) {
  if (event.button === 1 || (event.button === 0 && spaceDown.value)) {
    event.preventDefault()
    canvas.value!.setPointerCapture(event.pointerId)
    drag = { kind: 'pan', from: [event.clientX, event.clientY], ...shown() }
    panning.value = true
    return
  }
  if (!frame.value || event.button !== 0) return
  const point = cell(event)
  const [x, y] = point
  const id = tool.value
  if (id === 'picker') { pick(x, y, event.shiftKey); return }
  if (id === 'wand') {
    if (insideImage(x, y)) select(combine(selection.value, magicWand(size.value, frame.value, x, y, tolerance.value, contiguous.value), event))
    return
  }
  if (id === 'select') {
    canvas.value!.setPointerCapture(event.pointerId)
    if (selection.value && maskHas(size.value, selection.value, x, y) && !event.shiftKey && !event.altKey) {
      remember()
      const base = new Uint8ClampedArray(frame.value)
      erase(base, selection.value)
      drag = { kind: 'move', start: point, base, layer: lift(frame.value, selection.value), mask: selection.value }
    } else {
      drag = { kind: 'box', start: point, previous: selection.value, event: { shiftKey: event.shiftKey, altKey: event.altKey } }
      if (!event.shiftKey && !event.altKey) select(null)
    }
    return
  }
  if (id === 'fill') {
    if (!insideImage(x, y)) return
    remember(); fill(x, y); version.value++; return
  }
  canvas.value!.setPointerCapture(event.pointerId)
  if (id === 'gradient') {
    remember()
    // Inside a selection the gradient fills it; otherwise it fills the flat region clicked, like the fill bucket.
    const region = selection.value ?? magicWand(size.value, frame.value, clampX(x), clampY(y), 0, true)
    drag = { kind: 'gradient', start: point, base: new Uint8ClampedArray(frame.value), region }
    return
  }
  remember()
  painted = false
  if (id === 'line' || id === 'rect' || id === 'ellipse') {
    drag = { kind: 'shape', start: point, base: new Uint8ClampedArray(frame.value) }
    paintCells(shapeCells(id, point, point), strokeValue())
  } else {
    drag = { kind: 'stroke', last: point }
    paint(x, y)
  }
  painted = true
  version.value++
}
function move(event: PointerEvent) {
  const point = cell(event)
  const [x, y] = point
  hover.value = insideImage(x, y) ? [x, y] : null
  if (!drag || !frame.value) return
  if (drag.kind === 'pan') {
    view.value = { zoom: view.value.zoom, x: drag.x + event.clientX - drag.from[0], y: drag.y + event.clientY - drag.from[1] }
    return
  }
  if (drag.kind === 'stroke') {
    for (const [px, py] of linePoints(drag.last[0], drag.last[1], x, y)) paint(px, py)
    drag.last = point
  } else if (drag.kind === 'shape') {
    frame.value.set(drag.base)
    paintCells(shapeCells(tool.value, drag.start, point), strokeValue())
  } else if (drag.kind === 'gradient') {
    gradientAt(drag.base, drag.region, [clampX(drag.start[0]), clampY(drag.start[1])], [clampX(x), clampY(y)])
  } else if (drag.kind === 'box') {
    select(combine(drag.previous, rectMask(size.value, clampX(drag.start[0]), clampY(drag.start[1]), clampX(x), clampY(y)), drag.event))
  } else {
    const dx = x - drag.start[0], dy = y - drag.start[1]
    frame.value.set(drag.base)
    drop(size.value, frame.value, drag.layer, dx, dy)
    select(shiftMask(size.value, drag.mask, dx, dy))
  }
  version.value++
}
function up() {
  if (painted && strokeTools.includes(tool.value) && tool.value !== 'eraser' && !recent.value.includes(color.value)) recent.value = [color.value, ...recent.value].slice(0, 8)
  drag = null
  painted = false
  panning.value = false
}
const hover = ref<[number, number] | null>(null)

// ---- frames ----
function addFrame(copy: boolean) {
  if (props.frames.length >= props.maxFrames) return
  const next = copy && frame.value ? new Uint8ClampedArray(frame.value) : blankImage()
  props.frames.splice(frameIndex.value + 1, 0, next)
  frameIndex.value++
  version.value++
}
function deleteFrame() {
  if (props.frames.length <= 1) return
  props.frames.splice(frameIndex.value, 1)
  frameIndex.value = Math.max(0, frameIndex.value - 1)
  version.value++
}
function shiftFrame(delta: number) {
  const frames = props.frames
  const to = frameIndex.value + delta
  if (to < 0 || to >= frames.length) return
  ;[frames[frameIndex.value], frames[to]] = [frames[to]!, frames[frameIndex.value]!]
  frameIndex.value = to
  version.value++
}
function clearFrame() {
  if (!frame.value) return
  remember()
  frame.value.fill(0)
  version.value++
}

// ---- drawing to screen ----
const SCALE = 8
/** Paints an image as one square per pixel. Drawing one canvas onto another
 *  is avoided on purpose: WebKit (the macOS app's engine) can draw the source
 *  as it was before the latest putImageData, which showed stale or blank frames. */
function blit(target: CanvasRenderingContext2D, pixels: Uint8ClampedArray, cellSize: number, alpha = 1, ox = 0, oy = 0) {
  const w = props.width
  target.globalAlpha = alpha
  for (let i = 0; i < pixels.length / 4; i++) {
    const a = pixels[i * 4 + 3]!
    if (!a) continue
    target.fillStyle = `rgba(${pixels[i * 4]},${pixels[i * 4 + 1]},${pixels[i * 4 + 2]},${a / 255})`
    target.fillRect(ox + (i % w) * cellSize, oy + Math.floor(i / w) * cellSize, cellSize, cellSize)
  }
  target.globalAlpha = 1
}
const thumb = thumbnailer()
function render() {
  const ctx = canvas.value?.getContext('2d')
  if (!ctx) return
  const [w, h] = [props.width * SCALE, props.height * SCALE]
  ctx.clearRect(0, 0, w, h)
  const dark = document.documentElement.dataset.theme === 'dark'
  for (let y = 0; y < props.height; y += 2) for (let x = 0; x < props.width; x += 2) {
    ctx.fillStyle = (x + y) % 4 === 0 ? (dark ? '#262b29' : '#f3f2ec') : (dark ? '#2d3331' : '#e9e8e0')
    ctx.fillRect(x * SCALE, y * SCALE, SCALE * 2, SCALE * 2)
  }
  const previous = props.frames[frameIndex.value - 1]
  if (props.animation && onion.value && previous) blit(ctx, previous, SCALE, 0.25)
  if (frame.value) blit(ctx, frame.value, SCALE)
  if (grid.value) {
    ctx.strokeStyle = dark ? '#ffffff14' : '#0000000f'
    ctx.lineWidth = 1
    ctx.beginPath()
    for (let i = 1; i < props.width; i++) { ctx.moveTo(i * SCALE + 0.5, 0); ctx.lineTo(i * SCALE + 0.5, h) }
    for (let i = 1; i < props.height; i++) { ctx.moveTo(0, i * SCALE + 0.5); ctx.lineTo(w, i * SCALE + 0.5) }
    ctx.stroke()
    if (mirror.value) { ctx.strokeStyle = '#41a6f6aa'; ctx.beginPath(); ctx.moveTo(w / 2, 0); ctx.lineTo(w / 2, h); ctx.stroke() }
  }
  if (selection.value) outline(ctx, selection.value, dark)
  if (hover.value && tool.value !== 'fill') {
    ctx.strokeStyle = dark ? '#ffffffaa' : '#00000066'
    ctx.strokeRect(hover.value[0] * SCALE + 0.5, hover.value[1] * SCALE + 0.5, SCALE - 1, SCALE - 1)
  }
}
/** Traces the edge of the selection: dark underneath, light dashes on top, so it reads on any colour. */
function outline(ctx: CanvasRenderingContext2D, mask: Mask, dark: boolean) {
  const has = (x: number, y: number) => maskHas(size.value, mask, x, y)
  ctx.beginPath()
  for (let y = 0; y < props.height; y++) for (let x = 0; x < props.width; x++) {
    if (!mask[y * props.width + x]) continue
    const [l, t, r, b] = [x * SCALE, y * SCALE, (x + 1) * SCALE, (y + 1) * SCALE]
    if (!has(x, y - 1)) { ctx.moveTo(l, t); ctx.lineTo(r, t) }
    if (!has(x, y + 1)) { ctx.moveTo(l, b); ctx.lineTo(r, b) }
    if (!has(x - 1, y)) { ctx.moveTo(l, t); ctx.lineTo(l, b) }
    if (!has(x + 1, y)) { ctx.moveTo(r, t); ctx.lineTo(r, b) }
  }
  ctx.lineWidth = 2
  ctx.strokeStyle = dark ? '#000000cc' : '#000000aa'
  ctx.setLineDash([])
  ctx.stroke()
  ctx.lineWidth = 1
  ctx.strokeStyle = '#ffffff'
  ctx.setLineDash([4, 4])
  ctx.stroke()
  ctx.setLineDash([])
}
watch([version, frameIndex, onion, grid, mirror, hover, tool, selection], render)

// ---- preview ----
// Paused, the preview shows the frame being edited. It animates only while the
// user asks, so an open Studio costs no CPU.
const PREVIEW = 144
const preview = ref<HTMLCanvasElement | null>(null)
const playing = ref(false)
const canPlay = computed(() => props.frames.length > 1)
let tick = 0
let previewFrame = 0
let lastTime = 0
function showInPreview(pixels: Uint8ClampedArray | undefined) {
  const ctx = preview.value?.getContext('2d')
  if (!ctx) return
  ctx.clearRect(0, 0, PREVIEW, PREVIEW)
  const cellSize = PREVIEW / Math.max(props.width, props.height)
  if (pixels) blit(ctx, pixels, cellSize, 1, (PREVIEW - props.width * cellSize) / 2, (PREVIEW - props.height * cellSize) / 2)
}
function animate(time: number) {
  const frames = props.frames
  if (!playing.value || !frames.length) return
  tick = requestAnimationFrame(animate)
  if (time - lastTime < 1000 / props.fps) return
  lastTime = time
  if (previewFrame >= frames.length - 1 && !props.loop) { playing.value = false; return }
  previewFrame = (previewFrame + 1) % frames.length
  showInPreview(frames[previewFrame])
}
watch(playing, on => {
  cancelAnimationFrame(tick)
  if (on) { previewFrame = -1; lastTime = 0; tick = requestAnimationFrame(animate) }
  else showInPreview(frame.value)
})
watch([version, frameIndex, preview], () => { if (!playing.value) showInPreview(frame.value) })
watch(canPlay, () => { playing.value = false })

/** Another clip or prop, or a new size: start fresh on its first frame. */
watch([() => props.frames, () => props.width, () => props.height], async () => {
  frameIndex.value = 0
  selection.value = null
  playing.value = false
  hover.value = null
  fitView()
  await nextTick()
  render()
  showInPreview(frame.value)
})

function key(event: KeyboardEvent) {
  if ((event.target as HTMLElement).closest?.('input, textarea, [contenteditable]')) return
  const mod = event.metaKey || event.ctrlKey
  const lower = event.key.toLowerCase()
  if (mod && lower === 'z') { event.preventDefault(); event.shiftKey ? redo() : undo(); return }
  if (mod) {
    const action = { a: selectAll, c: () => copySelection(false), x: () => copySelection(true), v: paste, d: deselect }[lower]
    if (action) { event.preventDefault(); action() }
    return
  }
  const arrow = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[event.key]
  if (arrow) { event.preventDefault(); nudgeBy(arrow[0]! * (event.shiftKey ? 4 : 1), arrow[1]! * (event.shiftKey ? 4 : 1)); return }
  if (event.key === 'Delete' || event.key === 'Backspace') { event.preventDefault(); deleteSelection(); return }
  if (event.key === 'Escape') { deselect(); return }
  if (event.key === ' ') { event.preventDefault(); spaceDown.value = true; return }
  if (event.key === '+' || event.key === '=') { zoomBy(1.5); return }
  if (event.key === '-') { zoomBy(1 / 1.5); return }
  if (event.key === '0') { fitView(); return }
  if (lower === 'x') { swapColors(); return }
  const found = tools.find(t => t.key === event.key.toLowerCase())
  if (found) tool.value = found.id
  else if (event.key.toLowerCase() === 'm') mirror.value = !mirror.value
  else if (props.animation && event.key === ',') frameIndex.value = Math.max(0, frameIndex.value - 1)
  else if (props.animation && event.key === '.') frameIndex.value = Math.min(props.frames.length - 1, frameIndex.value + 1)
}
const keyUp = (event: KeyboardEvent) => { if (event.key === ' ') spaceDown.value = false }
onMounted(async () => {
  await nextTick(); render(); showInPreview(frame.value)
  window.addEventListener('keydown', key)
  window.addEventListener('keyup', keyUp)
})
onUnmounted(() => { cancelAnimationFrame(tick); observer?.disconnect(); window.removeEventListener('keydown', key); window.removeEventListener('keyup', keyUp) })
defineExpose({ version })
</script>

<template>
  <div class="sprite-editor" :class="{ solo: !slots.list }">
    <slot name="list" />

    <section class="canvas-area">
      <slot name="head" />

      <slot v-if="!frames.length" name="empty" />

      <template v-else>
        <div class="toolbar" role="toolbar" aria-label="Drawing tools">
          <template v-for="(t, i) in tools" :key="t.id">
            <span v-if="i > 0 && tools[i - 1]!.group !== t.group" class="rail-sep" />
            <button type="button" class="rail-button" :aria-pressed="tool === t.id" :title="`${t.label} (${t.key.toUpperCase()})`" :aria-label="t.label" @click="tool = t.id">
              <component :is="t.icon" :size="16" />
            </button>
          </template>
          <span class="rail-sep" />
          <button type="button" class="rail-button" :aria-pressed="mirror" title="Mirror: draw both sides at once (M)" aria-label="Mirror drawing" @click="mirror = !mirror"><FoldHorizontal :size="16" /></button>
          <button v-if="animation" type="button" class="rail-button" :aria-pressed="onion" title="Show the previous frame faintly" aria-label="Onion skin" @click="onion = !onion"><Layers :size="16" /></button>
          <button type="button" class="rail-button" :aria-pressed="grid" title="Grid" aria-label="Grid" @click="grid = !grid"><Grid3x3 :size="16" /></button>
          <span class="rail-sep" />
          <button type="button" class="rail-button" :disabled="!canUndo" title="Undo (⌘Z)" aria-label="Undo" @click="undo"><Undo2 :size="16" /></button>
          <button type="button" class="rail-button" :disabled="!canRedo" title="Redo (⇧⌘Z)" aria-label="Redo" @click="redo"><Redo2 :size="16" /></button>
          <span class="zoom-group">
            <button type="button" class="rail-button" :disabled="view.zoom <= 1" title="Zoom out (-)" aria-label="Zoom out" @click="zoomBy(1 / 1.5)"><ZoomOut :size="16" /></button>
            <button type="button" class="zoom-readout" title="Fit to window (0)" aria-label="Fit to window" @click="fitView">{{ Math.round(view.zoom * 100) }}%</button>
            <button type="button" class="rail-button" :disabled="view.zoom >= MAX_ZOOM" title="Zoom in (+). Scroll or pinch on the canvas works too." aria-label="Zoom in" @click="zoomBy(1.5)"><ZoomIn :size="16" /></button>
          </span>
        </div>

        <div class="tool-options" aria-label="Tool options">
          <label v-if="strokeTools.includes(tool)" class="option">Size
            <input v-model.number="brush" type="range" min="1" max="6" aria-label="Brush size"><span>{{ brush }}</span>
          </label>
          <label v-if="tool === 'rect' || tool === 'ellipse'" class="option"><input v-model="filledShapes" type="checkbox"> Filled</label>
          <template v-if="tool === 'gradient'">
            <span class="option segmented">
              <button type="button" class="chip" :aria-pressed="gradientKind === 'linear'" @click="gradientKind = 'linear'">Linear</button>
              <button type="button" class="chip" :aria-pressed="gradientKind === 'radial'" @click="gradientKind = 'radial'">Radial</button>
            </span>
            <span class="option segmented" title="What the gradient ends on">
              <button type="button" class="chip" :aria-pressed="gradientEnd === 'colour'" @click="gradientEnd = 'colour'">To 2nd colour</button>
              <button type="button" class="chip" :aria-pressed="gradientEnd === 'clear'" @click="gradientEnd = 'clear'">Fade out</button>
            </span>
            <label class="option" title="Two tones only, the classic pixel-art look"><input v-model="dither" type="checkbox"> Dither</label>
          </template>
          <template v-if="tool === 'wand'">
            <label class="option">Tolerance
              <input v-model.number="tolerance" type="range" min="0" max="128" aria-label="Wand tolerance"><span>{{ tolerance }}</span>
            </label>
            <label class="option"><input v-model="contiguous" type="checkbox"> Connected</label>
          </template>
          <span class="option actions">
            <template v-if="hasSelection">
              <button type="button" class="icon-button" title="Cut (⌘X)" aria-label="Cut" @click="copySelection(true)"><Scissors :size="15" /></button>
              <button type="button" class="icon-button" title="Copy (⌘C)" aria-label="Copy" @click="copySelection(false)"><Copy :size="15" /></button>
            </template>
            <button type="button" class="icon-button" title="Paste (⌘V)" aria-label="Paste" :disabled="!clipboard" @click="paste"><ClipboardPaste :size="15" /></button>
            <button type="button" class="icon-button" :title="hasSelection ? 'Flip the selection left to right' : 'Flip the image left to right'" aria-label="Flip horizontally" @click="flip('horizontal')"><FlipHorizontal2 :size="15" /></button>
            <button type="button" class="icon-button" :title="hasSelection ? 'Flip the selection upside down' : 'Flip the image upside down'" aria-label="Flip vertically" @click="flip('vertical')"><FlipVertical2 :size="15" /></button>
            <template v-if="hasSelection">
              <button type="button" class="text-link" @click="invertSelection">Invert</button>
              <button type="button" class="text-link" @click="deleteSelection">Delete</button>
              <button type="button" class="text-link" @click="deselect">Deselect</button>
            </template>
            <button v-else type="button" class="text-link" @click="selectAll">Select all</button>
            <button v-if="!animation" type="button" class="text-link" @click="clearFrame">Clear</button>
          </span>
        </div>

        <div ref="stage" class="canvas-stage" :class="{ panning: spaceDown || panning }">
          <canvas ref="canvas" class="pixel-canvas" :class="`tool-${tool}`" :width="width * SCALE" :height="height * SCALE" :style="canvasStyle"
                  aria-label="Drawing canvas" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="up" @pointerleave="hover = null" />
          <div v-if="slots.overlay" class="canvas-overlay" :style="canvasStyle"><slot name="overlay" :width="width" :height="height" /></div>
        </div>
        <div v-if="animation" class="frame-strip" aria-label="Frames">
          <button v-for="(f, i) in frames" :key="i" type="button" class="frame-thumb" :aria-pressed="frameIndex === i" :aria-label="`Frame ${i + 1}`" @click="frameIndex = i">
            <img v-if="thumb(f, width, height, version)" :src="thumb(f, width, height, version)" alt=""><span>{{ i + 1 }}</span>
          </button>
          <button type="button" class="frame-add" :disabled="frames.length >= maxFrames" title="Add a blank frame" aria-label="Add a blank frame" @click="addFrame(false)"><Plus :size="16" /></button>
          <div class="frame-tools">
            <button type="button" class="icon-button" title="Duplicate this frame" aria-label="Duplicate frame" :disabled="frames.length >= maxFrames" @click="addFrame(true)"><Copy :size="15" /></button>
            <button type="button" class="icon-button" title="Move frame left" aria-label="Move frame left" @click="shiftFrame(-1)"><ArrowLeft :size="15" /></button>
            <button type="button" class="icon-button" title="Move frame right" aria-label="Move frame right" @click="shiftFrame(1)"><ArrowRight :size="15" /></button>
            <button type="button" class="icon-button" title="Clear this frame" aria-label="Clear frame" @click="clearFrame"><Eraser :size="15" /></button>
            <button type="button" class="icon-button" title="Delete this frame" aria-label="Delete frame" :disabled="frames.length <= 1" @click="deleteFrame"><Trash2 :size="15" /></button>
          </div>
        </div>
      </template>
    </section>

    <aside class="side-panel">
      <div class="panel-block">
        <h3>Colour</h3>
        <div class="palette" role="radiogroup" aria-label="Palette">
          <button v-for="swatch in palette" :key="swatch" type="button" role="radio" class="paint" :style="{ background: swatch }"
                  :aria-checked="color === swatch && tool !== 'eraser'" :aria-label="swatch" :title="`${swatch} — shift-click for the second colour`" @click="useColor(swatch, $event.shiftKey)" />
        </div>
        <div class="colour-row">
          <label class="current-colour" :style="{ background: color }"><input type="color" :value="color" aria-label="Custom colour" @input="useColor(($event.target as HTMLInputElement).value)"></label>
          <span class="colour-hex">{{ color }}</span>
          <button type="button" class="icon-button" title="Swap the two colours (X)" aria-label="Swap colours" @click="swapColors"><ArrowLeftRight :size="14" /></button>
          <label class="current-colour second" :style="{ background: color2 }" title="Second colour: where a gradient ends"><input type="color" :value="color2" aria-label="Second colour" @input="color2 = ($event.target as HTMLInputElement).value"></label>
        </div>
        <div v-if="recent.length" class="recent">
          <button v-for="swatch in recent" :key="swatch" type="button" class="paint small" :style="{ background: swatch }" :aria-label="`Recent ${swatch}`" @click="useColor(swatch)" />
        </div>
      </div>
      <div v-if="animation && frames.length" class="panel-block">
        <div class="preview-head">
          <h3>Preview</h3>
          <button type="button" class="preview-play" :disabled="!canPlay" :aria-pressed="playing" :title="canPlay ? undefined : 'Add a second frame to play the animation'"
                  @click="playing = !playing"><Pause v-if="playing" :size="14" /><Play v-else :size="14" />{{ playing ? 'Pause' : 'Play' }}</button>
        </div>
        <div class="preview-stage"><canvas ref="preview" :width="PREVIEW" :height="PREVIEW" :aria-label="playing ? 'Animation preview, playing' : 'Animation preview, paused'" /></div>
        <label class="fps-row">Speed <input :value="fps" type="range" min="1" max="24" aria-label="Frames per second" @input="emit('update:fps', Number(($event.target as HTMLInputElement).value))"><span>{{ fps }} fps</span></label>
        <label class="check-row"><input :checked="loop" type="checkbox" @change="emit('update:loop', ($event.target as HTMLInputElement).checked)"> Loop</label>
      </div>
      <slot name="side" />
    </aside>
  </div>
</template>

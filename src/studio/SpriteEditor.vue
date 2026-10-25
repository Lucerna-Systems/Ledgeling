<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import { ArrowLeft, ArrowLeftRight, ArrowRight, Blend, BoxSelect, Circle, ClipboardPaste, Copy, Eraser, FlipHorizontal2, FlipVertical2, FoldHorizontal, Grid3x3, Layers, Minus, PaintBucket, Pause, Pencil, Pipette, Play, Plus, Redo2, Scissors, Square, Trash2, Undo2, WandSparkles, ZoomIn, ZoomOut } from '@lucide/vue'
import { FRAME, MAX_FRAMES, blankFrame, clipDrawn, clipInfo, clipsFor, drawn, emptyClip, type Draft } from './model'
import { applyGradient, combineMasks, drop, ellipsePoints, emptyMask, erase, flipPixels, invertMask, lift, linePoints, magicWand, maskEmpty, maskHas, nudge, opaqueMask, rectMask, rectPoints, shiftMask, thicken, type Mask, type Point, type Rgba } from './pixels'

const props = defineProps<{ draft: Draft }>()
/** Pixel edits are invisible to Vue (typed arrays are not reactive), so the
 *  Studio hears about them here to re-check what still needs drawing. */
const emit = defineEmits<{ edited: [] }>()
/** Bumped after every pixel change; thumbnails and the preview redraw from it. */
const version = ref(0)
watch(version, () => emit('edited'))

// ---- which animation and frame ----
const clipName = ref('idle')
const frameIndex = ref(0)
const clip = computed(() => props.draft.clips[clipName.value])
const frame = computed(() => clip.value?.frames[frameIndex.value])
const groups = computed(() => {
  const list = clipsFor(props.draft.movement)
  return [
    { title: 'Basics', items: list.filter(c => c.group === 'basics') },
    { title: 'Getting around', items: list.filter(c => c.group === 'moves') },
    { title: 'Moods', items: list.filter(c => c.group === 'moods') },
    { title: 'Helping you', items: list.filter(c => c.group === 'assistant') },
  ].filter(g => g.items.length)
})
/** The drawn animations a new one can be copied from: Idle first, then Move. */
const copySources = computed(() => ['idle', 'move'].filter(n => n !== clipName.value && clipDrawn(props.draft.clips[n])).map(n => clipInfo(n)!))
/** What plays in place of an undrawn clip: its fallback if drawn, else Idle/Move, whichever exists. */
function fallbackFor(name: string) {
  const wanted = clipInfo(name)?.fallback ?? 'idle'
  if (clipDrawn(props.draft.clips[wanted])) return wanted
  const other = wanted === 'idle' ? 'move' : 'idle'
  return clipDrawn(props.draft.clips[other]) ? other : wanted
}
function pickClip(name: string) { clipName.value = name; frameIndex.value = 0 }
function startClip(copyFrom?: string) {
  const source = copyFrom ? props.draft.clips[copyFrom] : undefined
  props.draft.clips[clipName.value] = source
    ? { frames: source.frames.map(f => new Uint8ClampedArray(f)), fps: source.fps, loop: true }
    : emptyClip()
  frameIndex.value = 0
  version.value++
}
function removeClip() {
  delete props.draft.clips[clipName.value]
  version.value++
}
// A movement change can hide the selected clip; fall back to idle.
watch(() => props.draft.movement, () => { if (!clipsFor(props.draft.movement).some(c => c.name === clipName.value)) pickClip('idle') })

// ---- zoom and pan ----
const MAX_ZOOM = 16
const stage = ref<HTMLElement | null>(null)
const stageSize = ref({ w: 480, h: 480 })
/** `x` and `y` are where the canvas's top-left sits in the stage; they only matter while zoomed in. */
const view = ref({ zoom: 1, x: 0, y: 0 })
const spaceDown = ref(false)
const panning = ref(false)
const baseSize = computed(() => Math.max(64, Math.min(stageSize.value.w, stageSize.value.h)))
/** Keeps a zoomed canvas covering the stage, and a fitted one centred. */
const clampAxis = (pos: number, size: number, dim: number) => size <= dim ? (dim - size) / 2 : Math.min(0, Math.max(dim - size, pos))
function shown() {
  const size = baseSize.value * view.value.zoom
  return { x: clampAxis(view.value.x, size, stageSize.value.w), y: clampAxis(view.value.y, size, stageSize.value.h) }
}
const canvasStyle = computed(() => {
  const size = baseSize.value * view.value.zoom
  const at = shown()
  return { width: `${size}px`, height: `${size}px`, left: `${at.x}px`, top: `${at.y}px` }
})
/** Zooms keeping the point under (cx, cy), in stage pixels, where it is. */
function zoomAt(factor: number, cx: number, cy: number) {
  const zoom = Math.min(MAX_ZOOM, Math.max(1, view.value.zoom * factor))
  const size = baseSize.value * view.value.zoom
  const at = shown()
  const [fx, fy] = [(cx - at.x) / size, (cy - at.y) / size]
  const next = baseSize.value * zoom
  view.value = { zoom, x: cx - fx * next, y: cy - fy * next }
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
watch(clipName, fitView)

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

// ---- undo: snapshots of the frame before each stroke ----
interface Snapshot { clip: string; index: number; pixels: Uint8ClampedArray }
const undoStack: Snapshot[] = []
const redoStack: Snapshot[] = []
const canUndo = ref(false)
const canRedo = ref(false)
function remember() {
  if (!frame.value) return
  undoStack.push({ clip: clipName.value, index: frameIndex.value, pixels: new Uint8ClampedArray(frame.value) })
  if (undoStack.length > 100) undoStack.shift()
  redoStack.length = 0
  canUndo.value = true; canRedo.value = false
}
function restore(from: Snapshot[], to: Snapshot[]) {
  const snap = from.pop()
  const target = snap && props.draft.clips[snap.clip]?.frames[snap.index]
  if (!snap || !target) return
  to.push({ ...snap, pixels: new Uint8ClampedArray(target) })
  target.set(snap.pixels)
  clipName.value = snap.clip; frameIndex.value = snap.index
  canUndo.value = undoStack.length > 0; canRedo.value = redoStack.length > 0
  version.value++
}
const undo = () => restore(undoStack, redoStack)
const redo = () => restore(redoStack, undoStack)

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
const allowed = (x: number, y: number) => !selection.value || maskHas(selection.value, x, y)
watch([clipName, frameIndex], () => { selection.value = null })
/** What copy and cut hold: a full-frame layer, transparent outside what was copied. */
const clipboard = shallowRef<Uint8ClampedArray | null>(null)

function selectAll() { select(rectMask(0, 0, FRAME - 1, FRAME - 1)) }
function deselect() { select(null) }
function invertSelection() { select(invertMask(selection.value ?? emptyMask())) }
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
  if (!layer) return
  edit(pixels => drop(pixels, layer, 0, 0))
  select(opaqueMask(layer))
  tool.value = 'select'
}
function flip(axis: 'horizontal' | 'vertical') { edit(pixels => flipPixels(pixels, axis, selection.value)) }
function nudgeBy(dx: number, dy: number) {
  edit(pixels => nudge(pixels, dx, dy, selection.value))
  if (selection.value) select(shiftMask(selection.value, dx, dy))
}

// ---- painting ----
function setPixel(pixels: Uint8ClampedArray, x: number, y: number, value: Rgba) {
  if (x < 0 || y < 0 || x >= FRAME || y >= FRAME || !allowed(x, y)) return
  pixels.set(value, (y * FRAME + x) * 4)
}
/** Paints cells with the brush, mirrored when mirror is on. */
function paintCells(points: Point[], value: Rgba) {
  const pixels = frame.value
  if (!pixels) return
  for (const [x, y] of thicken(points, brush.value)) {
    setPixel(pixels, x, y, value)
    if (mirror.value) setPixel(pixels, FRAME - 1 - x, y, value)
  }
}
const strokeValue = (): Rgba => tool.value === 'eraser' ? [0, 0, 0, 0] : rgba(color.value)
function paint(x: number, y: number) { paintCells([[x, y]], strokeValue()) }
function fill(x: number, y: number) {
  const pixels = frame.value
  if (!pixels) return
  const at = (px: number, py: number) => (py * FRAME + px) * 4
  const target = Array.from(pixels.subarray(at(x, y), at(x, y) + 4))
  const value = rgba(color.value)
  if (target.every((v, i) => v === value[i])) return
  const same = (i: number) => target.every((v, k) => pixels[i + k] === v)
  const queue = [[x, y]]
  while (queue.length) {
    const [px, py] = queue.pop()!
    if (px < 0 || py < 0 || px >= FRAME || py >= FRAME || !same(at(px, py)) || !allowed(px, py)) continue
    pixels.set(value, at(px, py))
    queue.push([px + 1, py], [px - 1, py], [px, py + 1], [px, py - 1])
  }
}
function pick(x: number, y: number, secondary = false) {
  const pixels = frame.value
  if (!pixels || x < 0 || y < 0 || x >= FRAME || y >= FRAME) return
  const i = (y * FRAME + x) * 4
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
  return [Math.floor(((event.clientX - box.left) / box.width) * FRAME), Math.floor(((event.clientY - box.top) / box.height) * FRAME)]
}
const clamp = (n: number) => Math.min(FRAME - 1, Math.max(0, n))
function shapeCells(kind: Tool, [x0, y0]: Point, [x1, y1]: Point): Point[] {
  if (kind === 'line') return linePoints(x0, y0, x1, y1)
  if (kind === 'rect') return rectPoints(x0, y0, x1, y1, filledShapes.value)
  return ellipsePoints(x0, y0, x1, y1, filledShapes.value)
}
function gradientAt(base: Uint8ClampedArray, region: Mask, from: Point, to: Point) {
  const pixels = frame.value!
  pixels.set(base)
  applyGradient(pixels, region, {
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
    if (x >= 0 && y >= 0 && x < FRAME && y < FRAME) select(combine(selection.value, magicWand(frame.value, x, y, tolerance.value, contiguous.value), event))
    return
  }
  if (id === 'select') {
    canvas.value!.setPointerCapture(event.pointerId)
    if (selection.value && maskHas(selection.value, x, y) && !event.shiftKey && !event.altKey) {
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
  if (id === 'fill') { remember(); fill(x, y); version.value++; return }
  canvas.value!.setPointerCapture(event.pointerId)
  if (id === 'gradient') {
    remember()
    // Inside a selection the gradient fills it; otherwise it fills the flat region clicked, like the fill bucket.
    const region = selection.value ?? magicWand(frame.value, clamp(x), clamp(y), 0, true)
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
  hover.value = x >= 0 && y >= 0 && x < FRAME && y < FRAME ? [x, y] : null
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
    gradientAt(drag.base, drag.region, [clamp(drag.start[0]), clamp(drag.start[1])], [clamp(x), clamp(y)])
  } else if (drag.kind === 'box') {
    select(combine(drag.previous, rectMask(clamp(drag.start[0]), clamp(drag.start[1]), clamp(x), clamp(y)), drag.event))
  } else {
    const dx = x - drag.start[0], dy = y - drag.start[1]
    frame.value.set(drag.base)
    drop(frame.value, drag.layer, dx, dy)
    select(shiftMask(drag.mask, dx, dy))
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
  if (!clip.value || clip.value.frames.length >= MAX_FRAMES) return
  const next = copy && frame.value ? new Uint8ClampedArray(frame.value) : blankFrame()
  clip.value.frames.splice(frameIndex.value + 1, 0, next)
  frameIndex.value++
  version.value++
}
function deleteFrame() {
  if (!clip.value || clip.value.frames.length <= 1) return
  clip.value.frames.splice(frameIndex.value, 1)
  frameIndex.value = Math.max(0, frameIndex.value - 1)
  version.value++
}
function shiftFrame(delta: number) {
  const frames = clip.value?.frames
  const to = frameIndex.value + delta
  if (!frames || to < 0 || to >= frames.length) return
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
/** Paints a frame as one square per pixel. Drawing one canvas onto another
 *  is avoided on purpose: WebKit (the macOS app's engine) can draw the source
 *  as it was before the latest putImageData, which showed stale or blank frames. */
function blit(target: CanvasRenderingContext2D, pixels: Uint8ClampedArray, size: number, alpha = 1) {
  const cell = size / FRAME
  target.globalAlpha = alpha
  for (let i = 0; i < FRAME * FRAME; i++) {
    const a = pixels[i * 4 + 3]!
    if (!a) continue
    target.fillStyle = `rgba(${pixels[i * 4]},${pixels[i * 4 + 1]},${pixels[i * 4 + 2]},${a / 255})`
    target.fillRect((i % FRAME) * cell, Math.floor(i / FRAME) * cell, cell, cell)
  }
  target.globalAlpha = 1
}
/** Thumbnails read their canvas back with toDataURL, which always sees the latest pixels. */
const thumbCanvas = document.createElement('canvas')
thumbCanvas.width = thumbCanvas.height = FRAME
function render() {
  const ctx = canvas.value?.getContext('2d')
  if (!ctx) return
  const size = FRAME * SCALE
  ctx.clearRect(0, 0, size, size)
  const dark = document.documentElement.dataset.theme === 'dark'
  for (let y = 0; y < FRAME; y += 2) for (let x = 0; x < FRAME; x += 2) {
    ctx.fillStyle = (x + y) % 4 === 0 ? (dark ? '#262b29' : '#f3f2ec') : (dark ? '#2d3331' : '#e9e8e0')
    ctx.fillRect(x * SCALE, y * SCALE, SCALE * 2, SCALE * 2)
  }
  const previous = clip.value?.frames[frameIndex.value - 1]
  if (onion.value && previous) blit(ctx, previous, size, 0.25)
  if (frame.value) blit(ctx, frame.value, size)
  if (grid.value) {
    ctx.strokeStyle = dark ? '#ffffff14' : '#0000000f'
    ctx.lineWidth = 1
    ctx.beginPath()
    for (let i = 1; i < FRAME; i++) { ctx.moveTo(i * SCALE + 0.5, 0); ctx.lineTo(i * SCALE + 0.5, size); ctx.moveTo(0, i * SCALE + 0.5); ctx.lineTo(size, i * SCALE + 0.5) }
    ctx.stroke()
    if (mirror.value) { ctx.strokeStyle = '#41a6f6aa'; ctx.beginPath(); ctx.moveTo(size / 2, 0); ctx.lineTo(size / 2, size); ctx.stroke() }
  }
  if (selection.value) outline(ctx, selection.value, dark)
  if (hover.value && tool.value !== 'fill') {
    ctx.strokeStyle = dark ? '#ffffffaa' : '#00000066'
    ctx.strokeRect(hover.value[0] * SCALE + 0.5, hover.value[1] * SCALE + 0.5, SCALE - 1, SCALE - 1)
  }
}
/** Traces the edge of the selection: dark underneath, light dashes on top, so it reads on any colour. */
function outline(ctx: CanvasRenderingContext2D, mask: Mask, dark: boolean) {
  ctx.beginPath()
  for (let y = 0; y < FRAME; y++) for (let x = 0; x < FRAME; x++) {
    if (!mask[y * FRAME + x]) continue
    const [l, t, r, b] = [x * SCALE, y * SCALE, (x + 1) * SCALE, (y + 1) * SCALE]
    if (!maskHas(mask, x, y - 1)) { ctx.moveTo(l, t); ctx.lineTo(r, t) }
    if (!maskHas(mask, x, y + 1)) { ctx.moveTo(l, b); ctx.lineTo(r, b) }
    if (!maskHas(mask, x - 1, y)) { ctx.moveTo(l, t); ctx.lineTo(l, b) }
    if (!maskHas(mask, x + 1, y)) { ctx.moveTo(r, t); ctx.lineTo(r, b) }
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
watch([version, clipName, frameIndex, onion, grid, mirror, hover, tool, selection], render)

const thumbCache = new Map<Uint8ClampedArray, { version: number; url: string }>()
function thumb(pixels: Uint8ClampedArray | undefined) {
  if (!pixels) return ''
  const cached = thumbCache.get(pixels)
  if (cached?.version === version.value) return cached.url
  thumbCanvas.getContext('2d')!.putImageData(new ImageData(new Uint8ClampedArray(pixels), FRAME, FRAME), 0, 0)
  const url = drawn(pixels) ? thumbCanvas.toDataURL() : ''
  thumbCache.set(pixels, { version: version.value, url })
  return url
}

// ---- preview ----
// Paused, the preview shows the frame being edited. It animates only while the
// user asks, so an open Studio costs no CPU.
const preview = ref<HTMLCanvasElement | null>(null)
const playing = ref(false)
const canPlay = computed(() => (clip.value?.frames.length ?? 0) > 1)
let tick = 0
let previewFrame = 0
let lastTime = 0
function showInPreview(pixels: Uint8ClampedArray | undefined) {
  const ctx = preview.value?.getContext('2d')
  if (!ctx) return
  ctx.clearRect(0, 0, 144, 144)
  if (pixels) blit(ctx, pixels, 144)
}
function animate(time: number) {
  const current = clip.value
  if (!playing.value || !current) return
  tick = requestAnimationFrame(animate)
  if (time - lastTime < 1000 / current.fps) return
  lastTime = time
  if (previewFrame >= current.frames.length - 1 && !current.loop) { playing.value = false; return }
  previewFrame = (previewFrame + 1) % current.frames.length
  showInPreview(current.frames[previewFrame])
}
watch(playing, on => {
  cancelAnimationFrame(tick)
  if (on) { previewFrame = -1; lastTime = 0; tick = requestAnimationFrame(animate) }
  else showInPreview(frame.value)
})
watch([version, clipName, frameIndex, preview], () => { if (!playing.value) showInPreview(frame.value) })
watch([clipName, canPlay], () => { playing.value = false })

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
  else if (event.key === ',' ) frameIndex.value = Math.max(0, frameIndex.value - 1)
  else if (event.key === '.' && clip.value) frameIndex.value = Math.min(clip.value.frames.length - 1, frameIndex.value + 1)
}
const keyUp = (event: KeyboardEvent) => { if (event.key === ' ') spaceDown.value = false }
onMounted(async () => {
  await nextTick(); render(); showInPreview(frame.value)
  window.addEventListener('keydown', key)
  window.addEventListener('keyup', keyUp)
})
onUnmounted(() => { cancelAnimationFrame(tick); observer?.disconnect(); window.removeEventListener('keydown', key); window.removeEventListener('keyup', keyUp) })
watch(clip, async () => { await nextTick(); render() })
defineExpose({ version })
</script>

<template>
  <div class="sprite-editor">
    <aside class="clip-list" aria-label="Animations">
      <template v-for="group in groups" :key="group.title">
        <h3>{{ group.title }}</h3>
        <button v-for="info in group.items" :key="info.name" type="button" class="clip-row" :class="{ selected: clipName === info.name }"
                :aria-pressed="clipName === info.name" @click="pickClip(info.name)">
          <span class="clip-thumb" :class="{ empty: !clipDrawn(draft.clips[info.name]) }">
            <img v-if="thumb(draft.clips[info.name]?.frames[0])" :src="thumb(draft.clips[info.name]?.frames[0])" alt="">
          </span>
          <span class="clip-text">
            <strong>{{ info.label }}</strong>
            <small v-if="clipDrawn(draft.clips[info.name])">{{ draft.clips[info.name]!.frames.length }} frame{{ draft.clips[info.name]!.frames.length === 1 ? '' : 's' }}</small>
            <small v-else>Uses {{ clipInfo(fallbackFor(info.name))?.label }}</small>
          </span>
        </button>
      </template>
    </aside>

    <section class="canvas-area">
      <header class="canvas-head">
        <div>
          <h2>{{ clipInfo(clipName)?.label }}</h2>
          <p>{{ clipInfo(clipName)?.blurb }}</p>
        </div>
        <button v-if="clip" type="button" class="text-link danger-link" @click="removeClip">Remove animation</button>
      </header>

      <div v-if="!clip" class="clip-start">
        <p>This animation is optional. Until you draw it, your pet plays <strong>{{ clipInfo(fallbackFor(clipName))?.label }}</strong> instead.</p>
        <div class="clip-start-actions">
          <button type="button" class="primary-button" @click="startClip()"><Plus :size="15" /> Start drawing</button>
          <button v-for="source in copySources" :key="source.name" type="button" @click="startClip(source.name)"><Copy :size="15" /> Start from {{ source.label }}</button>
        </div>
      </div>

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
          <button type="button" class="rail-button" :aria-pressed="onion" title="Show the previous frame faintly" aria-label="Onion skin" @click="onion = !onion"><Layers :size="16" /></button>
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
            <button type="button" class="icon-button" :title="hasSelection ? 'Flip the selection left to right' : 'Flip the frame left to right'" aria-label="Flip horizontally" @click="flip('horizontal')"><FlipHorizontal2 :size="15" /></button>
            <button type="button" class="icon-button" :title="hasSelection ? 'Flip the selection upside down' : 'Flip the frame upside down'" aria-label="Flip vertically" @click="flip('vertical')"><FlipVertical2 :size="15" /></button>
            <template v-if="hasSelection">
              <button type="button" class="text-link" @click="invertSelection">Invert</button>
              <button type="button" class="text-link" @click="deleteSelection">Delete</button>
              <button type="button" class="text-link" @click="deselect">Deselect</button>
            </template>
            <button v-else type="button" class="text-link" @click="selectAll">Select all</button>
          </span>
        </div>

        <div ref="stage" class="canvas-stage" :class="{ panning: spaceDown || panning }">
          <canvas ref="canvas" class="pixel-canvas" :class="`tool-${tool}`" :width="FRAME * SCALE" :height="FRAME * SCALE" :style="canvasStyle"
                  aria-label="Drawing canvas" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="up" @pointerleave="hover = null" />
        </div>
        <div class="frame-strip" aria-label="Frames">
          <button v-for="(f, i) in clip.frames" :key="i" type="button" class="frame-thumb" :aria-pressed="frameIndex === i" :aria-label="`Frame ${i + 1}`" @click="frameIndex = i">
            <img v-if="thumb(f)" :src="thumb(f)" alt=""><span>{{ i + 1 }}</span>
          </button>
          <button type="button" class="frame-add" :disabled="clip.frames.length >= MAX_FRAMES" title="Add a blank frame" aria-label="Add a blank frame" @click="addFrame(false)"><Plus :size="16" /></button>
          <div class="frame-tools">
            <button type="button" class="icon-button" title="Duplicate this frame" aria-label="Duplicate frame" :disabled="clip.frames.length >= MAX_FRAMES" @click="addFrame(true)"><Copy :size="15" /></button>
            <button type="button" class="icon-button" title="Move frame left" aria-label="Move frame left" @click="shiftFrame(-1)"><ArrowLeft :size="15" /></button>
            <button type="button" class="icon-button" title="Move frame right" aria-label="Move frame right" @click="shiftFrame(1)"><ArrowRight :size="15" /></button>
            <button type="button" class="icon-button" title="Clear this frame" aria-label="Clear frame" @click="clearFrame"><Eraser :size="15" /></button>
            <button type="button" class="icon-button" title="Delete this frame" aria-label="Delete frame" :disabled="clip.frames.length <= 1" @click="deleteFrame"><Trash2 :size="15" /></button>
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
      <div v-if="clip" class="panel-block">
        <div class="preview-head">
          <h3>Preview</h3>
          <button type="button" class="preview-play" :disabled="!canPlay" :aria-pressed="playing" :title="canPlay ? undefined : 'Add a second frame to play the animation'"
                  @click="playing = !playing"><Pause v-if="playing" :size="14" /><Play v-else :size="14" />{{ playing ? 'Pause' : 'Play' }}</button>
        </div>
        <div class="preview-stage"><canvas ref="preview" width="144" height="144" :aria-label="playing ? 'Animation preview, playing' : 'Animation preview, paused'" /></div>
        <label class="fps-row">Speed <input v-model.number="clip.fps" type="range" min="1" max="24" aria-label="Frames per second"><span>{{ clip.fps }} fps</span></label>
        <label class="check-row"><input v-model="clip.loop" type="checkbox"> Loop</label>
      </div>
    </aside>
  </div>
</template>

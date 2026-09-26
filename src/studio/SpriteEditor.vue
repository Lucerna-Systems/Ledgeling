<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { ArrowLeft, ArrowRight, Copy, Eraser, FlipHorizontal2, Grid3x3, Layers, PaintBucket, Pause, Pencil, Pipette, Play, Plus, Redo2, Trash2, Undo2 } from '@lucide/vue'
import { FRAME, MAX_FRAMES, blankFrame, clipDrawn, clipInfo, clipsFor, drawn, emptyClip, type Draft } from './model'

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
    { title: 'Must have', items: list.filter(c => c.group === 'basics') },
    { title: 'Getting around', items: list.filter(c => c.group === 'moves') },
    { title: 'Moods', items: list.filter(c => c.group === 'moods') },
    { title: 'Helping you', items: list.filter(c => c.group === 'assistant') },
  ].filter(g => g.items.length)
})
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
  if (clipInfo(clipName.value)?.required) return
  delete props.draft.clips[clipName.value]
  version.value++
}
// A movement change can hide the selected clip; fall back to idle.
watch(() => props.draft.movement, () => { if (!clipsFor(props.draft.movement).some(c => c.name === clipName.value)) pickClip('idle') })

// ---- tools ----
type Tool = 'pencil' | 'eraser' | 'fill' | 'picker'
const tool = ref<Tool>('pencil')
const tools: { id: Tool; label: string; key: string; icon: unknown }[] = [
  { id: 'pencil', label: 'Pencil', key: 'b', icon: Pencil },
  { id: 'eraser', label: 'Eraser', key: 'e', icon: Eraser },
  { id: 'fill', label: 'Fill', key: 'g', icon: PaintBucket },
  { id: 'picker', label: 'Pick a colour', key: 'i', icon: Pipette },
]
const mirror = ref(false)
const grid = ref(true)
const onion = ref(true)
/** Sweetie 16 by GrafxKid: a small, friendly palette that suits pixel pets. */
const palette = ['#1a1c2c', '#5d275d', '#b13e53', '#ef7d57', '#ffcd75', '#a7f070', '#38b764', '#257179', '#29366f', '#3b5dc9', '#41a6f6', '#73eff7', '#f4f4f4', '#94b0c2', '#566c86', '#333c57']
const color = ref('#ef7d57')
const recent = ref<string[]>([])
function rgba(hex: string): [number, number, number, number] {
  const v = hex.replace('#', '')
  return [parseInt(v.slice(0, 2), 16), parseInt(v.slice(2, 4), 16), parseInt(v.slice(4, 6), 16), 255]
}
const hex = (r: number, g: number, b: number) => `#${[r, g, b].map(n => n.toString(16).padStart(2, '0')).join('')}`
function useColor(value: string) {
  color.value = value
  if (tool.value === 'eraser' || tool.value === 'picker') tool.value = 'pencil'
}

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

// ---- painting ----
function setPixel(pixels: Uint8ClampedArray, x: number, y: number, value: [number, number, number, number]) {
  if (x < 0 || y < 0 || x >= FRAME || y >= FRAME) return
  pixels.set(value, (y * FRAME + x) * 4)
}
function paint(x: number, y: number) {
  const pixels = frame.value
  if (!pixels) return
  const value: [number, number, number, number] = tool.value === 'eraser' ? [0, 0, 0, 0] : rgba(color.value)
  setPixel(pixels, x, y, value)
  if (mirror.value) setPixel(pixels, FRAME - 1 - x, y, value)
}
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
    if (px < 0 || py < 0 || px >= FRAME || py >= FRAME || !same(at(px, py))) continue
    pixels.set(value, at(px, py))
    queue.push([px + 1, py], [px - 1, py], [px, py + 1], [px, py - 1])
  }
}
function pick(x: number, y: number) {
  const pixels = frame.value
  if (!pixels) return
  const i = (y * FRAME + x) * 4
  if (pixels[i + 3]) useColor(hex(pixels[i]!, pixels[i + 1]!, pixels[i + 2]!))
}

const canvas = ref<HTMLCanvasElement | null>(null)
let last: [number, number] | null = null
function cell(event: PointerEvent): [number, number] {
  const box = canvas.value!.getBoundingClientRect()
  return [Math.floor(((event.clientX - box.left) / box.width) * FRAME), Math.floor(((event.clientY - box.top) / box.height) * FRAME)]
}
function down(event: PointerEvent) {
  if (!frame.value || event.button !== 0) return
  const [x, y] = cell(event)
  if (tool.value === 'picker') { pick(x, y); return }
  remember()
  if (tool.value === 'fill') { fill(x, y); version.value++; return }
  canvas.value!.setPointerCapture(event.pointerId)
  last = [x, y]
  paint(x, y)
  version.value++
}
function move(event: PointerEvent) {
  const [x, y] = cell(event)
  hover.value = x >= 0 && y >= 0 && x < FRAME && y < FRAME ? [x, y] : null
  if (!last) return
  // Bresenham between samples so fast strokes stay continuous.
  let [x0, y0] = last
  const dx = Math.abs(x - x0), dy = -Math.abs(y - y0), sx = x0 < x ? 1 : -1, sy = y0 < y ? 1 : -1
  let err = dx + dy
  for (;;) {
    paint(x0, y0)
    if (x0 === x && y0 === y) break
    const e2 = 2 * err
    if (e2 >= dy) { err += dy; x0 += sx }
    if (e2 <= dx) { err += dx; y0 += sy }
  }
  last = [x, y]
  version.value++
}
function up() {
  if (last && tool.value !== 'eraser' && !recent.value.includes(color.value)) recent.value = [color.value, ...recent.value].slice(0, 8)
  last = null
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
  if (hover.value && tool.value !== 'fill') {
    ctx.strokeStyle = dark ? '#ffffffaa' : '#00000066'
    ctx.strokeRect(hover.value[0] * SCALE + 0.5, hover.value[1] * SCALE + 0.5, SCALE - 1, SCALE - 1)
  }
}
watch([version, clipName, frameIndex, onion, grid, mirror, hover, tool], render)

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
  if ((event.target as HTMLElement).closest('input, textarea, [contenteditable]')) return
  const mod = event.metaKey || event.ctrlKey
  if (mod && event.key.toLowerCase() === 'z') { event.preventDefault(); event.shiftKey ? redo() : undo(); return }
  if (mod) return
  const found = tools.find(t => t.key === event.key.toLowerCase())
  if (found) tool.value = found.id
  else if (event.key.toLowerCase() === 'm') mirror.value = !mirror.value
  else if (event.key === ',' ) frameIndex.value = Math.max(0, frameIndex.value - 1)
  else if (event.key === '.' && clip.value) frameIndex.value = Math.min(clip.value.frames.length - 1, frameIndex.value + 1)
}
onMounted(async () => {
  await nextTick(); render(); showInPreview(frame.value)
  window.addEventListener('keydown', key)
})
onUnmounted(() => { cancelAnimationFrame(tick); window.removeEventListener('keydown', key) })
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
            <small v-else-if="info.required" class="needed">Needs drawing</small>
            <small v-else>Uses {{ clipInfo(info.fallback!)?.label }}</small>
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
        <button v-if="clip && !clipInfo(clipName)?.required" type="button" class="text-link danger-link" @click="removeClip">Remove animation</button>
      </header>

      <div v-if="!clip" class="clip-start">
        <p>This animation is optional. Until you draw it, your pet plays <strong>{{ clipInfo(clipInfo(clipName)?.fallback ?? 'idle')?.label }}</strong> instead.</p>
        <div class="clip-start-actions">
          <button type="button" class="primary-button" @click="startClip()"><Plus :size="15" /> Start drawing</button>
          <button v-if="clipDrawn(draft.clips.idle)" type="button" @click="startClip('idle')"><Copy :size="15" /> Start from Idle</button>
        </div>
      </div>

      <template v-else>
        <div class="canvas-row">
          <div class="tool-rail" role="toolbar" aria-label="Drawing tools">
            <button v-for="t in tools" :key="t.id" type="button" class="rail-button" :aria-pressed="tool === t.id" :title="`${t.label} (${t.key.toUpperCase()})`" :aria-label="t.label" @click="tool = t.id">
              <component :is="t.icon" :size="17" />
            </button>
            <span class="rail-gap" />
            <button type="button" class="rail-button" :aria-pressed="mirror" title="Mirror drawing (M)" aria-label="Mirror drawing" @click="mirror = !mirror"><FlipHorizontal2 :size="17" /></button>
            <button type="button" class="rail-button" :aria-pressed="onion" title="Show the previous frame faintly" aria-label="Onion skin" @click="onion = !onion"><Layers :size="17" /></button>
            <button type="button" class="rail-button" :aria-pressed="grid" title="Grid" aria-label="Grid" @click="grid = !grid"><Grid3x3 :size="17" /></button>
            <span class="rail-gap" />
            <button type="button" class="rail-button" :disabled="!canUndo" title="Undo (⌘Z)" aria-label="Undo" @click="undo"><Undo2 :size="17" /></button>
            <button type="button" class="rail-button" :disabled="!canRedo" title="Redo (⇧⌘Z)" aria-label="Redo" @click="redo"><Redo2 :size="17" /></button>
          </div>
          <div class="canvas-stage">
            <canvas ref="canvas" class="pixel-canvas" :class="`tool-${tool}`" :width="FRAME * SCALE" :height="FRAME * SCALE"
                    aria-label="Drawing canvas" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="up" @pointerleave="hover = null" />
          </div>
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
                  :aria-checked="color === swatch && tool !== 'eraser'" :aria-label="swatch" @click="useColor(swatch)" />
        </div>
        <div class="colour-row">
          <label class="current-colour" :style="{ background: color }"><input type="color" :value="color" aria-label="Custom colour" @input="useColor(($event.target as HTMLInputElement).value)"></label>
          <span class="colour-hex">{{ color }}</span>
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

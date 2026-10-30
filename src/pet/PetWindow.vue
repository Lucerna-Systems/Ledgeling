<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import type { DecorPayload, Frame, PropAtlas, PropFrame, Scene, SpawnPayload } from '../types'

/** The overlay is a renderer and nothing else: it draws the frames Rust sends
 *  and forwards pointer events back. It never decides where a pet is. */
const stage = ref<HTMLDivElement>()
const debug = ref<Scene['debug']>(null)
const atlases = new Map<number, SpawnPayload>()
const nodes = new Map<number, HTMLDivElement>()
const decorAtlases = new Map<string, Record<string, PropAtlas>>()
const propNodes = new Map<string, HTMLDivElement>()
let propLayer: HTMLDivElement | null = null
const off: UnlistenFn[] = []
const indicators = new Map<number, string>()
const fading = new Map<number, ReturnType<typeof setTimeout>>()
let disposed = false
let dragging = false
let pressedAt = 0
let pressedX = 0
let pressedY = 0
let dragPet: number | null = null
let nativeDragging = false

function element(frame: Frame) {
  let node = nodes.get(frame.petId)
  if (!node) {
    node = document.createElement('div')
    node.className = 'pet-sprite'
    node.dataset.petId = String(frame.petId)
    stage.value?.appendChild(node)
    nodes.set(frame.petId, node)
  }
  return node
}

function draw(frame: Frame) {
  const spawn = atlases.get(frame.petId)
  const clip = spawn?.atlas[frame.clip] ?? Object.values(spawn?.atlas ?? {})[0]
  if (!clip) return
  const node = element(frame)
  const taskStatus = indicators.get(frame.petId)
  node.style.filter = taskStatus === 'waiting' ? 'drop-shadow(0 0 5px #d6a64f)' : taskStatus === 'running' || taskStatus === 'queued' ? 'drop-shadow(0 0 4px #73a1be)' : taskStatus === 'completed' ? 'drop-shadow(0 0 4px #8bae6c)' : '' 
  const left = frame.x - frame.width * frame.anchorX
  const top = frame.y - frame.height * frame.anchorY
  node.style.width = `${frame.width}px`
  node.style.height = `${frame.height}px`
  node.style.transformOrigin = `${frame.anchorX * 100}% ${frame.anchorY * 100}%`
  // Squash and lean are about the anchor (the feet), like the rotation.
  node.style.transform = `translate3d(${left}px, ${top}px, 0) rotate(${frame.rotation + frame.lean}deg) scale(${(frame.flipX ? -1 : 1) * frame.squashX}, ${frame.squashY})`
  node.style.opacity = String(frame.opacity)
  node.style.backgroundImage = `url(${clip.image})`
  node.style.backgroundSize = `${clip.frames * frame.width}px ${frame.height}px`
  node.style.backgroundPositionX = `${-frame.frameIndex * frame.width}px`
}

/** Biome props sit in their own layer under the pets and never take clicks. */
function drawProp(prop: PropFrame) {
  const clip = decorAtlases.get(prop.decor)?.[prop.prop]
  if (!clip) return
  let node = propNodes.get(prop.key)
  if (!node) {
    if (!propLayer) {
      propLayer = document.createElement('div')
      propLayer.className = 'prop-layer'
      stage.value?.prepend(propLayer)
    }
    node = document.createElement('div')
    node.className = 'prop-sprite'
    propLayer.appendChild(node)
    propNodes.set(prop.key, node)
  }
  const left = prop.x - prop.width * prop.anchorX
  const top = prop.y - prop.height
  node.style.width = `${prop.width}px`
  node.style.height = `${prop.height}px`
  // Sway and squash pivot on the foot, where the prop meets the window.
  node.style.transformOrigin = `${prop.anchorX * 100}% 100%`
  node.style.transform = `translate3d(${left}px, ${top}px, 0) rotate(${prop.rotation}deg) scale(${(prop.flip ? -1 : 1) * prop.squashX}, ${prop.squashY})`
  node.style.backgroundImage = `url(${clip.image})`
  node.style.backgroundSize = `${clip.frames * prop.width}px ${prop.height}px`
  node.style.backgroundPositionX = `${-prop.frameIndex * prop.width}px`
}

function render(scene: Scene) {
  debug.value = scene.debug
  const present = new Set(scene.frames.map(f => f.petId))
  for (const [id, node] of nodes) {
    if (!present.has(id)) { node.remove(); nodes.delete(id) }
  }
  scene.frames.forEach(draw)
  const props = scene.props ?? []
  const shown = new Set(props.map(p => p.key))
  for (const [key, node] of propNodes) {
    if (!shown.has(key)) { node.remove(); propNodes.delete(key) }
  }
  props.forEach(drawProp)
}

function despawn(petId: number) {
  atlases.delete(petId)
  nodes.get(petId)?.remove()
  nodes.delete(petId)
  clearIndicator(petId)
}

function clearIndicator(petId: number) {
  clearTimeout(fading.get(petId))
  fading.delete(petId)
  indicators.delete(petId)
}

/** The halo says a task is live. A finished one glows briefly and then stops:
 *  a permanent mark would keep claiming attention the pet no longer needs. */
function indicate(petId: number, status: string) {
  clearTimeout(fading.get(petId))
  fading.delete(petId)
  if (!['queued', 'running', 'waiting', 'cancelling', 'undoing'].includes(status)) {
    indicators.set(petId, 'completed')
    fading.set(petId, setTimeout(() => clearIndicator(petId), 6000))
    return
  }
  indicators.set(petId, status)
}

async function subscribe<T>(event: string, callback: (value: T) => void) {
  const stop = await listen<T>(event, e => callback(e.payload))
  if (disposed) stop(); else off.push(stop)
}

onMounted(async () => {
  await subscribe<SpawnPayload>('pet:spawn', payload => atlases.set(payload.petId, payload))
  await subscribe<number>('pet:despawn', despawn)
  await subscribe<DecorPayload>('decor:spawn', payload => decorAtlases.set(payload.decor, payload.atlas))
  await subscribe<Scene>('pet:scene', render)
  await subscribe<number>('pet:drag-ended', () => { dragging = false; nativeDragging = false; dragPet = null; caring = false; stopHold() })
  await subscribe<{petId: number | null; status: string}>('task:indicator', value => { if (value.petId !== null) indicate(value.petId, value.status) })
  await subscribe<string>('pet:sound', source => { void new Audio(source).play().catch(() => {}) })
  // The backend sends spawn payloads before this webview could listen; ask it
  // to re-send them now that every listener is registered.
  // Rust decides what reduced motion means; the overlay only reports it.
  const reduce = matchMedia('(prefers-reduced-motion: reduce)')
  const tell = () => { void invoke('set_reduced_motion', { on: reduce.matches }).catch(() => {}) }
  tell()
  reduce.addEventListener('change', tell)
  const forget = () => reduce.removeEventListener('change', tell)
  if (disposed) forget(); else off.push(forget)
  const monitor = new URLSearchParams(location.search).get('monitor')
  if (monitor !== null) await invoke('pet_surface_ready', { monitor: Number(monitor) })
})
onUnmounted(() => { disposed = true; off.forEach(stop => stop()); fading.forEach(clearTimeout); fading.clear() })

/** On a Wayland layer overlay the backend cannot read the pointer, so the
 *  overlay reports it: once per frame while it moves over a pet, and once when
 *  it leaves. The compositor only routes it here while it is over a pet. */
const reportsPointer = new URLSearchParams(location.search).has('pointer')
let pointerAt: [number, number] | null = null
let pointerQueued = false
function report(point: [number, number] | null) {
  if (!reportsPointer) return
  pointerAt = point
  if (pointerQueued) return
  pointerQueued = true
  requestAnimationFrame(() => {
    pointerQueued = false
    void invoke('pet_pointer', { point: pointerAt }).catch(() => {})
  })
}
function leave() { if (!dragging) report(null) }

function act(kind: string) { void invoke('pet_interact', { kind }).catch(() => {}) }

function ask(event: MouseEvent) {
  const id = (event.target as HTMLElement).closest<HTMLElement>('[data-pet-id]')?.dataset.petId
  if (id) void invoke('show_pet_bubble', { petId: Number(id) }).catch(() => {})
}

/** A press that stays put this long is petting, not a click or a drag. */
const HOLD_MS = 500
let holdTimer: ReturnType<typeof setTimeout> | null = null
let caring = false
function stopHold() { if (holdTimer) { clearTimeout(holdTimer); holdTimer = null } }

function down(event: PointerEvent) {
  if (event.button !== 0) return
  stopHold()
  holdTimer = setTimeout(() => {
    holdTimer = null
    if (!dragging || nativeDragging) return
    caring = true
    act('pet')
  }, HOLD_MS)
  dragging = true
  pressedAt = event.timeStamp
  pressedX = event.clientX; pressedY = event.clientY
  dragPet = Number((event.target as HTMLElement).closest<HTMLElement>('[data-pet-id]')?.dataset.petId) || null
  // Capture so the release still arrives if the pointer leaves the overlay.
  ;(event.target as HTMLElement).setPointerCapture?.(event.pointerId)
  // The grab picks the pet under the pointer, so it must know where that is.
  if (reportsPointer) {
    void invoke('pet_pointer', { point: [event.clientX, event.clientY] }).catch(() => {}).then(() => act('grab'))
  } else act('grab')
}
function move(event: PointerEvent) {
  report([event.clientX, event.clientY])
  if (!dragging || nativeDragging || !dragPet || event.shiftKey) return
  if (Math.hypot(event.clientX - pressedX, event.clientY - pressedY) < 12) return
  if (caring) return
  stopHold()
  nativeDragging = true
  // Where the native drag is unavailable the pet simply follows the cursor;
  // it is tried once per press, not on every move.
  void invoke('pet_folder_drag', { petId: dragPet }).catch(() => {})
}
function up(event: PointerEvent) {
  if (!dragging) return
  dragging = false
  nativeDragging = false
  stopHold()
  act('release')
  const wasCaring = caring
  caring = false
  if (!wasCaring && event.timeStamp - pressedAt < 250) act('click')
}
</script>

<template>
  <div
    ref="stage"
    class="pet-stage"
    @pointerdown="down"
    @pointermove="move"
    @pointerup="up"
    @pointercancel="up"
    @pointerleave="leave"
    @contextmenu.prevent="ask"
  />
  <svg v-if="debug" class="debug-overlay" xmlns="http://www.w3.org/2000/svg">
    <line v-for="(s, i) in debug.surfaces" :key="`s${i}`" :x1="s[0]" :y1="s[1]" :x2="s[2]" :y2="s[3]" class="debug-surface" />
    <line v-for="(j, i) in debug.jumps" :key="`j${i}`" :x1="j[0]" :y1="j[1]" :x2="j[2]" :y2="j[3]" class="debug-jump" />
    <polyline v-for="(p, i) in debug.paths" :key="`p${i}`" :points="p.map(q => q.join(',')).join(' ')" class="debug-path" />
    <text v-for="(l, i) in debug.labels" :key="`l${i}`" :x="l.x + 10" :y="l.y - 8" class="debug-label">{{ l.text }}</text>
  </svg>
</template>

<style>
.pet-stage { position: fixed; inset: 0; overflow: hidden; }
.pet-sprite {
  position: absolute; left: 0; top: 0; will-change: transform;
  background-repeat: no-repeat; image-rendering: pixelated; user-select: none;
}
.prop-layer { position: absolute; inset: 0; pointer-events: none; }
.prop-sprite {
  position: absolute; left: 0; top: 0; will-change: transform;
  background-repeat: no-repeat; image-rendering: pixelated;
}
.debug-overlay { position: fixed; inset: 0; width: 100%; height: 100%; pointer-events: none; }
.debug-surface { stroke: #4fbaa4; stroke-width: 3; }
.debug-jump { stroke: #d8a15a; stroke-width: 1.5; stroke-dasharray: 5 5; }
.debug-path { fill: none; stroke: #c7607a; stroke-width: 2; stroke-dasharray: 3 4; }
.debug-label { fill: #f4f7ee; font: 11px -apple-system, sans-serif; paint-order: stroke; stroke: #2c3a2ecc; stroke-width: 3px; }
</style>

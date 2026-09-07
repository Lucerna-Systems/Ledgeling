<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import type { Frame, Scene, SpawnPayload } from '../types'

/** The overlay is a renderer and nothing else: it draws the frames Rust sends
 *  and forwards pointer events back. It never decides where a pet is. */
const stage = ref<HTMLDivElement>()
const debug = ref<Scene['debug']>(null)
const atlases = new Map<number, SpawnPayload>()
const nodes = new Map<number, HTMLDivElement>()
const off: UnlistenFn[] = []
let disposed = false
let dragging = false
let pressedAt = 0

function element(frame: Frame) {
  let node = nodes.get(frame.petId)
  if (!node) {
    node = document.createElement('div')
    node.className = 'pet-sprite'
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
  const left = frame.x - frame.width * frame.anchorX
  const top = frame.y - frame.height * frame.anchorY
  node.style.width = `${frame.width}px`
  node.style.height = `${frame.height}px`
  node.style.transformOrigin = `${frame.anchorX * 100}% ${frame.anchorY * 100}%`
  node.style.transform = `translate3d(${left}px, ${top}px, 0) rotate(${frame.rotation}deg) scaleX(${frame.flipX ? -1 : 1})`
  node.style.opacity = String(frame.opacity)
  node.style.backgroundImage = `url(${clip.image})`
  node.style.backgroundSize = `${clip.frames * frame.width}px ${frame.height}px`
  node.style.backgroundPositionX = `${-frame.frameIndex * frame.width}px`
}

function render(scene: Scene) {
  debug.value = scene.debug
  const present = new Set(scene.frames.map(f => f.petId))
  for (const [id, node] of nodes) {
    if (!present.has(id)) { node.remove(); nodes.delete(id) }
  }
  scene.frames.forEach(draw)
}

function despawn(petId: number) {
  atlases.delete(petId)
  nodes.get(petId)?.remove()
  nodes.delete(petId)
}

async function subscribe<T>(event: string, callback: (value: T) => void) {
  const stop = await listen<T>(event, e => callback(e.payload))
  if (disposed) stop(); else off.push(stop)
}

onMounted(async () => {
  await subscribe<SpawnPayload>('pet:spawn', payload => atlases.set(payload.petId, payload))
  await subscribe<number>('pet:despawn', despawn)
  await subscribe<Scene>('pet:scene', render)
  await subscribe<string>('pet:sound', source => { void new Audio(source).play().catch(() => {}) })
  // The backend sends spawn payloads before this webview could listen; ask it
  // to re-send them now that every listener is registered.
  const monitor = new URLSearchParams(location.search).get('monitor')
  if (monitor !== null) await invoke('pet_surface_ready', { monitor: Number(monitor) })
})
onUnmounted(() => { disposed = true; off.forEach(stop => stop()) })

function act(kind: string) { void invoke('pet_interact', { kind }).catch(() => {}) }

function down(event: PointerEvent) {
  if (event.button !== 0) return
  dragging = true
  pressedAt = event.timeStamp
  // Capture so the release still arrives if the pointer leaves the overlay.
  ;(event.target as HTMLElement).setPointerCapture?.(event.pointerId)
  act('grab')
}
function up(event: PointerEvent) {
  if (!dragging) return
  dragging = false
  act('release')
  if (event.timeStamp - pressedAt < 250) act('click')
}
</script>

<template>
  <div
    ref="stage"
    class="pet-stage"
    @pointerdown="down"
    @pointerup="up"
    @pointercancel="up"
    @contextmenu.prevent="act('click')"
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
.debug-overlay { position: fixed; inset: 0; width: 100%; height: 100%; pointer-events: none; }
.debug-surface { stroke: #4fbaa4; stroke-width: 3; }
.debug-jump { stroke: #d8a15a; stroke-width: 1.5; stroke-dasharray: 5 5; }
.debug-path { fill: none; stroke: #c7607a; stroke-width: 2; stroke-dasharray: 3 4; }
.debug-label { fill: #f4f7ee; font: 11px -apple-system, sans-serif; paint-order: stroke; stroke: #2c3a2ecc; stroke-width: 3px; }
</style>

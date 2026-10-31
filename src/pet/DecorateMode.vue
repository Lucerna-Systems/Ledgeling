<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { EditView, PaletteItem, PlacedProp, PropAtlas } from '../types'
import { BAND, fractionAlong, inBand, thumbScale } from './decorate'

/** Decorate mode on the overlay. It shows where things can go and reports
 *  what the pointer did; Rust decides, saves and draws the props themselves. */
const props = defineProps<{
  edit: EditView
  atlas: (decor: string, prop: string) => PropAtlas | undefined
}>()

type Drag = { item: PaletteItem; index: number | null }
const drag = ref<Drag | null>(null)
const pointer = ref({ x: 0, y: 0 })

function send(change: Record<string, unknown>) { void invoke('biome_decorate', { change }).catch(() => {}) }
function done() { drag.value = null; void invoke('biome_decorate_end').catch(() => {}) }

function pick(event: PointerEvent) {
  if (!props.edit.picking || event.button !== 0) return
  void invoke('biome_pick', { point: [event.clientX, event.clientY] }).catch(() => {})
}

function itemFor(placed: PlacedProp) {
  return props.edit.palette.find(item => item.decor === placed.decor && item.prop === placed.prop)
}

function begin(event: PointerEvent, item: PaletteItem | undefined, index: number | null) {
  if (!item || event.button !== 0) return
  drag.value = { item, index }
  track(event)
}
function track(event: PointerEvent) { pointer.value = { x: event.clientX, y: event.clientY } }
function release(event: PointerEvent) {
  const current = drag.value
  drag.value = null
  // Dropped anywhere but the strip above the edge: nothing changes.
  if (!current || !inBand(event.clientX, event.clientY, props.edit)) return
  const x = fractionAlong(event.clientX, props.edit)
  if (current.index === null) send({ kind: 'place', decor: current.item.decor, prop: current.item.prop, x })
  else send({ kind: 'move', index: current.index, x })
}

function sprite(item: PaletteItem, scale: number) {
  const clip = props.atlas(item.decor, item.prop)
  if (!clip) return {}
  return {
    width: `${item.width * scale}px`,
    height: `${item.height * scale}px`,
    backgroundImage: `url(${clip.image})`,
    backgroundSize: `${clip.frames * item.width * scale}px ${item.height * scale}px`,
  }
}

const band = computed(() => ({
  left: `${props.edit.edgeX}px`,
  top: `${props.edit.edgeY - BAND}px`,
  width: `${props.edit.edgeWidth}px`,
  height: `${BAND}px`,
}))

/** Above the drop strip when there is room, else just under the window's edge. */
const tray = computed(() => {
  const above = props.edit.edgeY - BAND - 96
  const top = above > 8 ? above : props.edit.edgeY + 20
  const left = Math.min(Math.max(8, props.edit.edgeX), Math.max(8, window.innerWidth - 420))
  return { left: `${left}px`, top: `${top}px` }
})

function handle(placed: PlacedProp) {
  const item = itemFor(placed)
  if (!item) return { display: 'none' }
  const x = props.edit.edgeX + placed.x * props.edit.edgeWidth
  return { left: `${x - item.width / 2}px`, top: `${props.edit.edgeY - item.height}px`, width: `${item.width}px`, height: `${item.height}px` }
}

const ghost = computed(() => {
  const current = drag.value
  if (!current) return {}
  const { width, height } = current.item
  return {
    ...sprite(current.item, 1),
    left: `${pointer.value.x - width / 2}px`,
    top: `${pointer.value.y - height}px`,
    opacity: inBand(pointer.value.x, pointer.value.y, props.edit) ? '0.9' : '0.45',
  }
})

function onKey(event: KeyboardEvent) { if (event.key === 'Escape') done() }
onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div class="decorate" :class="{ picking: edit.picking }" @pointerdown.self="pick" @pointermove="track" @pointerup="release" @contextmenu.prevent>
    <div v-if="edit.picking" class="decorate-card decorate-ask">
      <p><strong>Click the window you want to decorate.</strong></p>
      <p>Its app gets a biome that shows on whichever of its windows is in front.</p>
      <p v-if="edit.error" class="decorate-error">{{ edit.error }}</p>
      <button type="button" @click="done">Cancel</button>
    </div>
    <template v-else>
      <div class="decorate-band" :style="band" />
      <div v-for="(placed, index) in edit.props" :key="`${index}-${placed.prop}`" class="decorate-handle" :style="handle(placed)"
           :aria-label="`${placed.prop}: drag to move, right-click to remove, double-click to flip`"
           @pointerdown.stop="begin($event, itemFor(placed), index)"
           @contextmenu.prevent.stop="send({ kind: 'remove', index })"
           @dblclick="send({ kind: 'flip', index })" />
      <div class="decorate-card decorate-tray" :style="tray" @pointerdown.stop>
        <div class="decorate-head">
          <strong>Decorating {{ edit.app }}</strong>
          <button type="button" class="decorate-done" @click="done">Done</button>
        </div>
        <div class="decorate-palette">
          <button v-for="item in edit.palette" :key="`${item.decor}/${item.prop}`" type="button" class="decorate-item" :title="item.prop"
                  :aria-label="`Add ${item.prop}`" @pointerdown.prevent="begin($event, item, null)">
            <span class="decorate-thumb" :style="sprite(item, thumbScale(item.width, item.height))" />
          </button>
        </div>
        <p class="decorate-hint">Drag onto the window's top edge. Right-click a prop to remove it, double-click to flip it.</p>
        <p v-if="edit.error" class="decorate-error">{{ edit.error }}</p>
      </div>
      <div v-if="drag" class="decorate-ghost" :style="ghost" />
    </template>
  </div>
</template>

<style>
.decorate { position: fixed; inset: 0; cursor: default; font: 13px -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; color: #1d2330; }
.decorate.picking { background: rgba(20, 24, 32, 0.18); cursor: crosshair; }
.decorate-card { position: absolute; background: rgba(255, 255, 255, 0.97); border-radius: 12px; box-shadow: 0 8px 30px rgba(0, 0, 0, 0.22); padding: 12px 14px; }
.decorate-card p { margin: 0 0 6px; max-width: 46ch; }
.decorate-ask { left: 50%; top: 38%; transform: translate(-50%, -50%); cursor: default; }
.decorate-tray { width: 400px; max-width: calc(100vw - 16px); box-sizing: border-box; }
.decorate-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.decorate-palette { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 8px; }
.decorate-item { width: 44px; height: 44px; display: grid; place-items: center; border: 1px solid #d9dce3; border-radius: 8px; background: #f6f7f9; cursor: grab; padding: 0; }
.decorate-item:hover { border-color: #5b5bd6; }
.decorate-thumb, .decorate-ghost { background-repeat: no-repeat; image-rendering: pixelated; }
.decorate-ghost { position: absolute; pointer-events: none; }
.decorate-band { position: absolute; border: 2px dashed rgba(91, 91, 214, 0.75); border-bottom: 0; border-radius: 10px 10px 0 0; background: rgba(91, 91, 214, 0.06); pointer-events: none; }
.decorate-handle { position: absolute; cursor: grab; border-radius: 6px; }
.decorate-handle:hover { outline: 2px solid rgba(91, 91, 214, 0.8); }
.decorate-hint { color: #5f6879; font-size: 12px; }
.decorate-error { color: #b4232c; }
.decorate button { font: inherit; }
.decorate-done, .decorate-ask button { border: 0; border-radius: 8px; padding: 5px 12px; background: #5b5bd6; color: #fff; cursor: pointer; }
@media (prefers-color-scheme: dark) {
  .decorate { color: #eef0f4; }
  .decorate-card { background: rgba(32, 36, 44, 0.97); }
  .decorate-item { background: #2a2f39; border-color: #3b414d; }
  .decorate-hint { color: #a6adbb; }
  .decorate-error { color: #ff8a8f; }
}
</style>

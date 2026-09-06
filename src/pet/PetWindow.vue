<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import type { Frame, Snapshot } from '../types'
const sprite = ref<HTMLDivElement>()
const world = ref<Snapshot>()
const error = ref('')
const off: UnlistenFn[] = []
let disposed = false
let held = false
async function act(kind: string) {
  try { await invoke('pet_action', { kind }) } catch (e) { error.value = String(e) }
}
function render(frame: Frame) {
  if (!sprite.value) return
  const el = sprite.value
  el.style.transform = `translate3d(${frame.x}px, ${frame.y}px, 0)`
  const bird = el.firstElementChild as HTMLElement
  bird.style.backgroundImage = `url(/${frame.clip}.png)`
  bird.style.backgroundPositionX = `${-frame.frameIndex * 128}px`
  bird.style.transform = `scaleX(${frame.flipX ? -1 : 1})`
  el.classList.toggle('greeting', !!frame.greeting)
}
async function subscribe<T>(event: string, callback: (value: T) => void) {
  const stop = await listen<T>(event, e => callback(e.payload))
  if (disposed) stop(); else off.push(stop)
}
onMounted(async () => {
  await subscribe<Frame>('pet:frame', render)
  await subscribe<Snapshot>('world:debug', value => world.value = value)
})
onUnmounted(() => { disposed = true; off.forEach(stop => stop()) })
function grab(e: PointerEvent) {
  if (e.button !== 0) return
  held = true
  void act('grab')
}
function release() { if (held) { held = false; void act('release') } }
</script>
<template>
  <svg v-if="world?.debug" class="surface-overlay" :viewBox="`${world.screen.x} ${world.screen.y} ${world.screen.width} ${world.screen.height}`" preserveAspectRatio="none">
    <g v-for="(surface, i) in world.surfaces" :key="i">
      <line :x1="surface.start" :x2="surface.end" :y1="surface.y" :y2="surface.y" stroke="#4fbaa4" stroke-width="3" />
      <circle :cx="surface.start" :cy="surface.y" r="5" fill="#4fbaa4" />
    </g>
  </svg>
  <div ref="sprite" class="pet-position" @pointerdown="grab" @pointerup="release" @dblclick="act('greet')" @contextmenu.prevent="act('greet')">
    <div class="pet-pixels" />
    <span class="pet-heart" aria-hidden="true">♥</span>
  </div>
  <span v-if="error" class="sr-only">{{ error }}</span>
</template>
<style>
.pet-position { position:absolute; left:0; top:0; width:128px; height:128px; will-change:transform; user-select:none; cursor:grab; }
.pet-position:active { cursor:grabbing; }
.pet-pixels { width:128px; height:128px; background:url('/idle.png') 0 0 / 512px 128px; image-rendering:pixelated; }
.pet-heart { position:absolute; left:88px; top:2px; font-size:19px; color:#bd7360; opacity:0; }
.greeting .pet-heart { opacity:1; }
.surface-overlay { position:fixed; inset:0; width:100%; height:100%; pointer-events:none; }
</style>

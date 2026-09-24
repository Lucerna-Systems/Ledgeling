<script setup lang="ts">
import { computed } from 'vue'
import { Check, Monitor } from '@lucide/vue'
import { action, busy, saveSettings, settings, snapshot } from '../state'
import type { ScreenView } from '../types'

const view = computed(() => {
  const bounds = snapshot.value?.bounds
  return bounds ? `${bounds.x} ${bounds.y} ${bounds.width} ${bounds.height}` : '0 0 100 100'
})
// Surface strokes follow the kind: top edges one colour, under-edges another,
// sides a third. Classes rather than inline fills so the map tokens in
// style.css can flip with the theme.
const edgeClass: Record<string, string> = {
  window_top: 'edge-top', taskbar_top: 'edge-top', monitor_bottom: 'edge-top',
  window_bottom: 'edge-under', monitor_top: 'edge-under',
  window_left: 'edge-side', window_right: 'edge-side', monitor_left: 'edge-side', monitor_right: 'edge-side',
}
const screens = computed(() => snapshot.value?.screens ?? [])
// The select shows the screen pets are actually on. A saved pick can name a
// display that just went away for the moment before the backend rewrites it,
// and it is not one of the options, so it would render blank.
const selected = computed(() => snapshot.value?.petScreen ?? settings.value?.petScreen ?? '')
function label(screen: ScreenView, index: number) {
  const size = `${Math.round(screen.bounds.width / screen.scale)}×${Math.round(screen.bounds.height / screen.scale)}`
  return `${screen.isPrimary ? 'Main display' : `Display ${index + 1}`} · ${size}`
}
function pick(event: Event) {
  const name = (event.target as HTMLSelectElement).value
  if (settings.value && name) void saveSettings({ ...settings.value, petScreen: name })
}
</script>

<template>
  <section class="page-heading">
    <div>
      <div class="eyebrow">A BIRD'S-EYE VIEW</div>
      <h1>Desktop view</h1>
      <p>Visible window edges become places to stand. Hidden ones do not.</p>
    </div>
  </section>

  <section class="map-card">
    <div class="map-toolbar">
      <span><span class="live-dot" />{{ snapshot?.screens.length ?? 0 }} display(s) · pets stay on one</span>
      <span>{{ snapshot?.windows.length ?? 0 }} windows · {{ snapshot?.perchCount ?? 0 }} window edges · {{ snapshot?.surfaces.length ?? 0 }} surfaces</span>
    </div>
    <div v-if="screens.length > 1" class="screen-picker">
      <label for="pet-screen">Pets live on</label>
      <select id="pet-screen" :value="selected" :disabled="busy" @change="pick">
        <option v-for="(screen, index) in screens" :key="screen.name" :value="screen.name">
          {{ label(screen, index) }}
        </option>
      </select>
    </div>
    <svg v-if="snapshot" class="desktop-map" :viewBox="view" role="img" aria-label="Live window geometry and available surfaces">
      <rect v-for="m in snapshot.monitors" :key="`m${m.id}`" class="map-monitor" :x="m.bounds.x" :y="m.bounds.y" :width="m.bounds.width" :height="m.bounds.height" />
      <rect v-for="win in [...snapshot.windows].reverse()" :key="win.id" class="map-window" :x="win.rect.x" :y="win.rect.y" :width="win.rect.width" :height="win.rect.height" rx="10" />
      <line v-for="(s, i) in snapshot.surfaces" :key="i" :class="edgeClass[s.kind] ?? 'edge-fallback'" :x1="s.x1" :y1="s.y1" :x2="s.x2" :y2="s.y2" stroke-width="6" stroke-linecap="round" />
      <circle v-for="pet in snapshot.pets" :key="pet.id" class="map-pet" :cx="pet.x" :cy="pet.y - 18" r="16" />
    </svg>
    <div v-else class="map-empty"><Monitor :size="40" :stroke-width="1" /><p>Your live desktop will appear in the native app.</p></div>
    <div class="map-legend">
      <span><i class="edge-top legend" /> Stand on top</span>
      <span><i class="edge-side legend" /> Climb the sides</span>
      <span><i class="edge-under legend" /> Hang underneath</span>
      <span><i class="map-pet legend" /> A pet</span>
    </div>
  </section>

  <div class="desktop-details">
    <div><h3>Draw it on the desktop</h3><p>Show the same surfaces, jump edges and paths over your real windows.</p></div>
    <button class="toggle" :class="{ enabled: snapshot?.settings.debug }" role="switch" :aria-checked="!!snapshot?.settings.debug" aria-label="Debug overlay" @click="action('debug')"><span /></button>
  </div>
  <div class="diagnostic-note">
    <Check :size="16" />
    <span>Window scan {{ snapshot ? `${snapshot.scanMs.toFixed(2)} ms` : '—' }} · geometry only, never window contents{{ snapshot?.suspended ? ' · suspended for a fullscreen app' : '' }}</span>
  </div>
</template>
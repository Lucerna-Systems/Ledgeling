<script setup lang="ts">
import { computed } from 'vue'
import { Check, Monitor } from '@lucide/vue'
import { action, snapshot } from '../state'

const view = computed(() => {
  const bounds = snapshot.value?.bounds
  return bounds ? `${bounds.x} ${bounds.y} ${bounds.width} ${bounds.height}` : '0 0 100 100'
})
const colours: Record<string, string> = {
  window_top: '#6d9569', taskbar_top: '#6d9569', monitor_bottom: '#6d9569',
  window_bottom: '#8aa9c0', monitor_top: '#8aa9c0',
  window_left: '#c9a24d', window_right: '#c9a24d', monitor_left: '#c9a24d', monitor_right: '#c9a24d',
}
</script>

<template>
  <section class="page-heading">
    <div>
      <div class="eyebrow">A BIRD'S-EYE VIEW</div>
      <h1>Your desktop.<br>Their little world<span>.</span></h1>
      <p>Visible window edges become places to stand. Hidden ones do not.</p>
    </div>
  </section>

  <section class="map-card">
    <div class="map-toolbar">
      <span><span class="live-dot" />{{ snapshot?.monitors.length ?? 0 }} display(s)</span>
      <span>{{ snapshot?.windows.length ?? 0 }} windows · {{ snapshot?.perchCount ?? 0 }} window edges · {{ snapshot?.surfaces.length ?? 0 }} surfaces</span>
    </div>
    <svg v-if="snapshot" class="desktop-map" :viewBox="view" role="img" aria-label="Live window geometry and available surfaces">
      <rect v-for="m in snapshot.monitors" :key="`m${m.id}`" :x="m.bounds.x" :y="m.bounds.y" :width="m.bounds.width" :height="m.bounds.height" fill="#edece2" stroke="#dcded2" stroke-width="4" />
      <rect v-for="win in [...snapshot.windows].reverse()" :key="win.id" :x="win.rect.x" :y="win.rect.y" :width="win.rect.width" :height="win.rect.height" rx="10" fill="#fbfaf5" stroke="#d4d8cb" stroke-width="3" />
      <line v-for="(s, i) in snapshot.surfaces" :key="i" :x1="s.x1" :y1="s.y1" :x2="s.x2" :y2="s.y2" :stroke="colours[s.kind] ?? '#9aa08e'" stroke-width="6" stroke-linecap="round" />
      <circle v-for="pet in snapshot.pets" :key="pet.id" :cx="pet.x" :cy="pet.y - 18" r="16" fill="#c78e52" stroke="#fff" stroke-width="4" />
    </svg>
    <div v-else class="map-empty"><Monitor :size="40" :stroke-width="1" /><p>Your live desktop will appear in the native app.</p></div>
    <div class="map-legend">
      <span><i style="background:#6d9569" /> Stand on top</span>
      <span><i style="background:#c9a24d" /> Climb the sides</span>
      <span><i style="background:#8aa9c0" /> Hang underneath</span>
      <span><i style="background:#c78e52" /> A pet</span>
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

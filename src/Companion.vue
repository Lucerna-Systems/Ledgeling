<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { Bird, Leaf, ArrowUpRight, Pause, Play, Eye, EyeOff, MoveUpRight, MousePointer2, ShieldCheck, SlidersHorizontal, X, Home, ScanLine, Power, ChevronRight, Monitor, Check, Heart } from '@lucide/vue'
import type { Snapshot } from './types'
const native = isTauri()
const state = ref<Snapshot | null>(null)
const page = ref<'companion' | 'desktop' | 'about'>('companion')
const error = ref('')
const busy = ref(false)
const speed = ref(220)
const adjustingSpeed = ref(false)
let timer: ReturnType<typeof setInterval> | undefined
let fetching = false
const status = computed(() => !native ? 'Browser preview' : !state.value ? 'Connecting…' : !state.value.visible ? 'Taking a little break' : state.value.paused ? 'Resting here' : ({ perched: 'Happily perched', flying: 'Finding a new favorite spot', greeting: 'Happy to see you', held: 'Along for the ride' }[state.value.activity]))
const perches = computed(() => state.value?.surfaces.filter(s => s.owner !== 0).length ?? 0)
async function refresh() {
  if (!native || fetching || busy.value || document.hidden) return
  fetching = true
  try { state.value = await invoke<Snapshot>('get_snapshot'); if (!adjustingSpeed.value) speed.value = state.value.speed }
  catch (e) { error.value = String(e) }
  finally { fetching = false }
}
async function action(kind: string) {
  if (!native || busy.value) return
  busy.value = true; error.value = ''
  try { state.value = await invoke<Snapshot>('pet_action', { kind }) }
  catch (e) { error.value = String(e) }
  finally { busy.value = false }
}
async function saveSpeed() {
  try { await invoke('set_speed', { speed: Number(speed.value) }) }
  catch (e) { error.value = String(e) }
  finally { adjustingSpeed.value = false }
}
async function quit() { try { await invoke('quit_app') } catch (e) { error.value = String(e) } }
onMounted(() => { void refresh(); timer = setInterval(refresh, 1000) })
onUnmounted(() => clearInterval(timer))
</script>
<template>
  <div class="app-shell">
    <aside class="sidebar">
      <a class="brand" href="#" @click.prevent="page = 'companion'"><span class="brand-mark"><Bird :size="25" :stroke-width="1.5" /></span>perch<span class="brand-dot">.</span></a>
      <div class="workspace-label">A LITTLE COMPANY</div>
      <nav aria-label="Main navigation">
        <button :class="{ selected: page === 'companion' }" @click="page = 'companion'"><Bird :size="18" /> Your companion</button>
        <button :class="{ selected: page === 'desktop' }" @click="page = 'desktop'"><Monitor :size="18" /> Desktop view</button>
        <button :class="{ selected: page === 'about' }" @click="page = 'about'"><ShieldCheck :size="18" /> Quiet by design</button>
      </nav>
      <div class="sidebar-bottom"><div class="small-sprig"><Leaf :size="24" :stroke-width="1.2" /></div><p>Make a little room<br>for a little friend.</p><span>PERCH · EARLY PREVIEW 0.1</span></div>
    </aside>
    <main class="content">
      <header class="topbar"><span>{{ page === 'companion' ? 'Your companion' : page === 'desktop' ? 'Desktop view' : 'Quiet by design' }}</span><span class="local-badge"><span /> All local. Always.</span></header>
      <div v-if="!native" class="preview-notice"><Monitor :size="16" /><span>This is a browser preview. Run <code>npm run tauri dev</code> to bring Pip onto your desktop.</span></div>
      <div v-if="error || state?.scanError || state?.renderError" class="error-banner" role="alert"><span>{{ error || state?.scanError || state?.renderError }}</span><button v-if="error" @click="error = ''" aria-label="Dismiss error"><X :size="16" /></button></div>
      <template v-if="page === 'companion'">
        <section class="page-heading"><div><div class="eyebrow">SMALL FRIEND. BIG PERSONALITY.</div><h1>A little company<br>for your day<span>.</span></h1><p>Meet Pip. Your desktop is their new favorite place.</p></div><span class="handwritten">oh, hello there <span>↙</span></span></section>
        <div class="companion-grid">
          <section class="pet-card">
            <div class="card-overline"><span class="soft-pill"><span /> {{ native ? 'YOUR DESKTOP BUDDIE' : 'MEET YOUR BUDDIE' }}</span><span class="card-number">01 / 01</span></div>
            <div class="bird-scene" aria-label="Pip, a green and yellow pixel budgie">
              <div class="scene-orbit orbit-one" /><div class="scene-orbit orbit-two" />
              <span class="scene-spark spark-one">✧</span><span class="scene-spark spark-two">✦</span>
              <div class="little-window back-window"><div class="window-dots"><i /><i /><i /></div><div class="window-lines"><i /><i /><i /></div></div>
              <div class="little-window front-window"><div class="window-dots"><i /><i /><i /></div><div class="window-lines"><i /><i /></div></div>
              <div class="hero-bird" :class="{ 'bird-resting': state?.paused }" />
              <span class="scene-leaf"><Leaf :size="40" :stroke-width="1" /></span>
              <span class="scene-caption">a very good window companion</span>
            </div>
            <div class="pet-identity"><div><div class="pet-name">Pip <span>THE BUDGIE</span></div><p>Curious by nature. Excellent at just being here.</p></div><span class="species-icon"><Bird :size="22" :stroke-width="1.4" /></span></div>
            <div class="pet-status" aria-live="polite"><span :class="{ sleeping: state?.paused || !state?.visible }" />{{ status }}</div>
          </section>
          <section class="controls-card">
            <div class="section-label"><SlidersHorizontal :size="16" /> A little fine-tuning</div>
            <h2>Let Pip settle in.</h2><p class="control-intro">They’ll flit between your windows and find a place to perch.</p>
            <button class="primary-button" :disabled="!state || busy" @click="action('relocate')"><MoveUpRight :size="18" /> Find a new perch <ArrowUpRight :size="16" class="ml-auto" /></button>
            <div class="paired-buttons"><button :disabled="!state || busy" @click="action('pause')"><Play v-if="state?.paused" :size="16" /><Pause v-else :size="16" />{{ state?.paused ? 'Resume' : 'Pause' }}</button><button :disabled="!state || busy" @click="action('visibility')"><EyeOff v-if="state?.visible ?? true" :size="16" /><Eye v-else :size="16" />{{ state?.visible ?? true ? 'Hide Pip' : 'Show Pip' }}</button></div>
            <div class="speed-control"><label for="flight-speed">Flight speed <span>{{ speed < 180 ? 'Unhurried' : speed > 300 ? 'Adventurous' : 'Easygoing' }}</span></label><input id="flight-speed" v-model="speed" type="range" min="100" max="400" step="10" :disabled="!state" @input="adjustingSpeed = true" @change="saveSpeed"><div class="range-labels"><span>Take it slow</span><span>Wings out</span></div></div>
            <button class="home-button" :disabled="!state || busy" @click="action('home')"><Home :size="15" /> Bring Pip to the desktop edge <ChevronRight :size="14" /></button>
            <div class="control-note"><Leaf :size="16" /><p>No tasks. No streaks.<br>Just a little presence.</p></div>
          </section>
        </div>
        <section class="tips-row"><div class="tip"><span class="tip-icon"><MousePointer2 :size="20" :stroke-width="1.4" /></span><div><h3>Say a little hello</h3><p>Drag Pip to pick them up.<br>Right-click for a happy hello.</p></div></div><div class="tip"><span class="tip-icon"><ScanLine :size="20" :stroke-width="1.4" /></span><div><h3>Your windows, their world</h3><p>{{ native ? `${perches} visible window perches to explore.` : 'Real window edges become little perches.' }}<br><button class="text-link" @click="page = 'desktop'">Take a look <ArrowUpRight :size="12" /></button></p></div></div></section>
      </template>
      <template v-else-if="page === 'desktop'">
        <section class="page-heading"><div><div class="eyebrow">A BIRD’S-EYE VIEW</div><h1>Your desktop.<br>Their little world<span>.</span></h1><p>Visible window tops become places to land.</p></div></section>
        <section class="map-card"><div class="map-toolbar"><span><span class="live-dot" />{{ native ? 'Live primary display' : 'Desktop preview' }}</span><span>{{ state?.windows.length ?? 0 }} windows · {{ perches }} perches</span></div>
          <svg v-if="state" class="desktop-map" :viewBox="`${state.screen.x} ${state.screen.y} ${state.screen.width} ${state.screen.height}`" role="img" aria-label="Live window geometry and available perches">
            <rect :x="state.screen.x" :y="state.screen.y" :width="state.screen.width" :height="state.screen.height" fill="#edece2" />
            <rect v-for="win in [...state.windows].reverse()" :key="win.id" :x="win.rect.x" :y="win.rect.y" :width="win.rect.width" :height="win.rect.height" rx="12" fill="#fbfaf5" stroke="#d4d8cb" stroke-width="3" />
            <line v-for="(surface,i) in state.surfaces" :key="i" :x1="surface.start" :x2="surface.end" :y1="surface.y" :y2="surface.y" stroke="#6d9569" stroke-width="7" stroke-linecap="round" />
            <circle v-if="state.visible" :cx="state.x" :cy="state.y - 20 * state.scale" :r="12 * state.scale" fill="#c78e52" stroke="#fff" stroke-width="4" />
          </svg><div v-else class="map-empty"><Monitor :size="40" :stroke-width="1" /><p>Your live desktop will appear in the native app.</p></div>
          <div class="map-legend"><span><i class="legend-perch" /> Available perch</span><span><i class="legend-pip" /> Pip</span><span class="ml-auto">Geometry only. No window contents.</span></div>
        </section>
        <div class="desktop-details"><div><h3>See what Pip sees</h3><p>Draw detected perches directly on your desktop.</p></div><button class="toggle" :class="{ enabled: state?.debug }" role="switch" :aria-checked="!!state?.debug" aria-label="Show surface overlay" :disabled="!state || busy" @click="action('debug')"><span /></button></div>
        <div class="diagnostic-note"><Check :size="16" /><span>Window scan {{ state ? `${state.scanMs.toFixed(2)} ms` : '—' }} · Primary display MVP · {{ state?.renderer === 'small' ? 'Small pet window' : 'Transparent overlay' }}</span></div>
      </template>
      <template v-else>
        <section class="page-heading"><div><div class="eyebrow">LESS NOISE. MORE TRUST.</div><h1>A companion.<br>Nothing to keep up with<span>.</span></h1><p>A small, local app with a small, clear purpose.</p></div></section>
        <section class="privacy-card"><ShieldCheck :size="34" :stroke-width="1.2" /><h2>Your day stays yours.</h2><p>Pip only needs the positions and sizes of your windows to find a perch. Perch doesn’t read window titles, screen contents, documents, or what you type.</p><div class="privacy-points"><span><Check :size="16" /> No accounts or sign-in</span><span><Check :size="16" /> No telemetry or cloud services</span><span><Check :size="16" /> No screen recording permission</span><span><Check :size="16" /> No reminders or notifications</span></div></section>
        <section class="about-note"><Heart :size="20" :stroke-width="1.4" /><div><h3>A small beginning</h3><p>This MVP gives Pip real window perches on your Mac’s primary display. More animals, pet packs, and session awareness are future milestones. Controls apply to this session.</p></div></section>
        <button class="quit-button" :disabled="!native" @click="quit"><Power :size="15" /> Quit Perch</button>
      </template>
      <footer><span><Leaf :size="13" /> A softer corner of your desktop.</span><span>Made for the in-between moments.</span></footer>
    </main>
  </div>
</template>

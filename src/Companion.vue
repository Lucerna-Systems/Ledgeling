<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { Bird, Leaf, Pause, Play, Eye, EyeOff, MoveUpRight, MousePointer2, ShieldCheck, SlidersHorizontal, X, ScanLine, Monitor, Package, FlaskConical, Activity, ArrowUpRight } from '@lucide/vue'
import { action, error, busy, native, notes, pets, saveSettings, settings, snapshot, start } from './state'
import PacksPanel from './panels/PacksPanel.vue'
import LabPanel from './panels/LabPanel.vue'
import AwarenessPanel from './panels/AwarenessPanel.vue'
import DesktopPanel from './panels/DesktopPanel.vue'
import PrivacyPanel from './panels/PrivacyPanel.vue'

type Page = 'companion' | 'packs' | 'desktop' | 'lab' | 'awareness' | 'about'
const page = ref<Page>('companion')
const speed = ref(1)
const adjusting = ref(false)
let stop = () => {}

const titles: Record<Page, string> = {
  companion: 'Your companions', packs: 'Pet packs', desktop: 'Desktop view',
  lab: 'Pet Lab', awareness: 'Your day', about: 'Quiet by design',
}

const status = computed(() => {
  if (!native) return 'Browser preview'
  if (!snapshot.value) return 'Connecting…'
  if (snapshot.value.suspended) return 'Out of the way for a fullscreen app'
  if (snapshot.value.settings.hidden) return 'Taking a little break'
  if (snapshot.value.settings.paused) return 'Resting here'
  if (!pets.value.length) return 'Nobody is out yet'
  const pet = pets.value[0]
  return `${pet.name} is ${pet.state.replace(/_/g, ' ')}`
})

onMounted(() => {
  stop = start()
  const timer = setInterval(() => {
    if (!adjusting.value && settings.value) speed.value = settings.value.speedScale
  }, 1000)
  onUnmounted(() => clearInterval(timer))
})
onUnmounted(() => stop())

function saveSpeed() {
  adjusting.value = false
  if (settings.value) void saveSettings({ ...settings.value, speedScale: Number(speed.value) })
}
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <a class="brand" href="#" @click.prevent="page = 'companion'"><span class="brand-mark"><Bird :size="25" :stroke-width="1.5" /></span>perch<span class="brand-dot">.</span></a>
      <div class="workspace-label">A LITTLE COMPANY</div>
      <nav aria-label="Main navigation">
        <button :class="{ selected: page === 'companion' }" @click="page = 'companion'"><Bird :size="18" /> Your companions</button>
        <button :class="{ selected: page === 'packs' }" @click="page = 'packs'"><Package :size="18" /> Pet packs</button>
        <button :class="{ selected: page === 'desktop' }" @click="page = 'desktop'"><Monitor :size="18" /> Desktop view</button>
        <button :class="{ selected: page === 'awareness' }" @click="page = 'awareness'"><Activity :size="18" /> Your day</button>
        <button :class="{ selected: page === 'lab' }" @click="page = 'lab'"><FlaskConical :size="18" /> Pet Lab</button>
        <button :class="{ selected: page === 'about' }" @click="page = 'about'"><ShieldCheck :size="18" /> Quiet by design</button>
      </nav>
      <div class="sidebar-bottom">
        <div class="small-sprig"><Leaf :size="24" :stroke-width="1.2" /></div>
        <p>Make a little room<br>for a little friend.</p>
        <span>PERCH · EARLY PREVIEW 0.1</span>
      </div>
    </aside>

    <main class="content">
      <header class="topbar">
        <span>{{ titles[page] }}</span>
        <span class="local-badge"><span /> All local. Always.</span>
      </header>
      <div v-if="!native" class="preview-notice"><Monitor :size="16" /><span>This is a browser preview. Run <code>npm run tauri dev</code> to bring a pet onto your desktop.</span></div>
      <div v-for="note in notes" :key="note" class="preview-notice"><ShieldCheck :size="16" /><span>{{ note }}</span></div>
      <div v-if="error || snapshot?.scanError" class="error-banner" role="alert">
        <span>{{ error || snapshot?.scanError }}</span>
        <button v-if="error" aria-label="Dismiss" @click="error = ''"><X :size="16" /></button>
      </div>

      <template v-if="page === 'companion'">
        <section class="page-heading">
          <div>
            <div class="eyebrow">SMALL FRIENDS. BIG PERSONALITIES.</div>
            <h1>A little company<br>for your day<span>.</span></h1>
            <p>They live on the edges of your windows. That is the whole idea.</p>
          </div>
          <span class="handwritten">oh, hello there <span>↙</span></span>
        </section>

        <div class="companion-grid">
          <section class="pet-card">
            <div class="card-overline">
              <span class="soft-pill"><span /> {{ pets.length ? 'ON YOUR DESKTOP' : 'NOBODY OUT YET' }}</span>
              <span class="card-number">{{ String(pets.length).padStart(2, '0') }} / {{ String(snapshot?.packs.length ?? 0).padStart(2, '0') }}</span>
            </div>
            <ul class="pet-list">
              <li v-for="pet in pets" :key="pet.id">
                <div>
                  <div class="pet-name">{{ pet.name }} <span>{{ pet.class.toUpperCase() }}</span></div>
                  <p>{{ pet.state.replace(/_/g, ' ') }}<span v-if="pet.surface"> · on the {{ pet.surface.replace(/_/g, ' ') }}</span></p>
                </div>
                <div class="pet-bars">
                  <label>energy<i><b :style="{ width: `${pet.energy * 100}%` }" /></i></label>
                  <label>mood<i><b :style="{ width: `${pet.mood * 100}%` }" /></i></label>
                </div>
              </li>
              <li v-if="!pets.length" class="pet-empty">
                <p>Open <button class="text-link" @click="page = 'packs'">Pet packs <ArrowUpRight :size="12" /></button> and bring one out.</p>
              </li>
            </ul>
            <div class="pet-status" aria-live="polite"><span :class="{ sleeping: settings?.paused || settings?.hidden }" />{{ status }}</div>
          </section>

          <section class="controls-card">
            <div class="section-label"><SlidersHorizontal :size="16" /> A little fine-tuning</div>
            <h2>Let them settle in.</h2>
            <p class="control-intro">They will find their own way between your windows.</p>
            <button class="primary-button" :disabled="!snapshot || busy" @click="action('relocate')"><MoveUpRight :size="18" /> Send everyone somewhere new</button>
            <div class="paired-buttons">
              <button :disabled="!snapshot || busy" @click="action('pause')">
                <Play v-if="settings?.paused" :size="16" /><Pause v-else :size="16" />{{ settings?.paused ? 'Resume' : 'Pause' }}
              </button>
              <button :disabled="!snapshot || busy" @click="action('visibility')">
                <EyeOff v-if="!settings?.hidden" :size="16" /><Eye v-else :size="16" />{{ settings?.hidden ? 'Show' : 'Hide' }}
              </button>
            </div>
            <div class="speed-control">
              <label for="speed">Pace <span>{{ speed < 0.8 ? 'Unhurried' : speed > 1.4 ? 'Adventurous' : 'Easygoing' }}</span></label>
              <input id="speed" v-model="speed" type="range" min="0.25" max="3" step="0.05" :disabled="!snapshot" @input="adjusting = true" @change="saveSpeed">
              <div class="range-labels"><span>Take it slow</span><span>Off they go</span></div>
            </div>
            <div class="control-note"><Leaf :size="16" /><p>No tasks. No streaks.<br>Just a little presence.</p></div>
          </section>
        </div>

        <section class="tips-row">
          <div class="tip">
            <span class="tip-icon"><MousePointer2 :size="20" :stroke-width="1.4" /></span>
            <div><h3>Say a little hello</h3><p>Drag a pet to pick it up.<br>A quick click says hello.</p></div>
          </div>
          <div class="tip">
            <span class="tip-icon"><ScanLine :size="20" :stroke-width="1.4" /></span>
            <div><h3>Your windows, their world</h3><p>{{ snapshot?.perchCount ?? 0 }} visible window edges right now.<br><button class="text-link" @click="page = 'desktop'">Take a look <ArrowUpRight :size="12" /></button></p></div>
          </div>
        </section>
      </template>

      <PacksPanel v-else-if="page === 'packs'" />
      <DesktopPanel v-else-if="page === 'desktop'" />
      <AwarenessPanel v-else-if="page === 'awareness'" />
      <LabPanel v-else-if="page === 'lab'" />
      <PrivacyPanel v-else />

      <footer>
        <span><Leaf :size="13" /> A softer corner of your desktop.</span>
        <span>Made for the in-between moments.</span>
      </footer>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import CollectionToolbar from './components/CollectionToolbar.vue'
import { listen } from '@tauri-apps/api/event'
import AssistantPanel from './assistant/AssistantPanel.vue'
import { startTasks, assignedPet, droppedFiles } from './assistant/state'
import { Bird, Leaf, Pause, Play, Eye, EyeOff, MoveUpRight, MousePointer2, ShieldCheck, SlidersHorizontal, X, ScanLine, Monitor, Package, FlaskConical, Activity, ArrowUpRight } from '@lucide/vue'
import { action, error, busy, native, notes, pets, packs, saveSettings, settings, snapshot, start } from './state'
import PacksPanel from './panels/PacksPanel.vue'
import LabPanel from './panels/LabPanel.vue'
import AwarenessPanel from './panels/AwarenessPanel.vue'
import DesktopPanel from './panels/DesktopPanel.vue'
import PrivacyPanel from './panels/PrivacyPanel.vue'

type Page = 'assistant' | 'companion' | 'packs' | 'desktop' | 'lab' | 'awareness' | 'about'
const page = ref<Page>('companion')
const assistantVisited = ref(false)
watch(page, value => { if (value === 'assistant') assistantVisited.value = true })
const content = ref<HTMLElement | null>(null)
watch(page, async () => { await nextTick(); if (content.value) content.value.scrollTop = 0 })
const petQuery = ref('')
const petClass = ref('all')
const petFilters = [
  { value: 'all', label: 'All companions' },
  ...['flier', 'climber', 'walker', 'hopper'].map(value => ({ value, label: value[0]!.toUpperCase() + value.slice(1) })),
]
const visiblePets = computed(() => pets.value.filter(pet =>
  (petClass.value === 'all' || pet.class === petClass.value) &&
  pet.name.toLowerCase().includes(petQuery.value.trim().toLowerCase())))
const previews = computed(() => new Map(packs.value.map(pack => [pack.summary.id, pack.summary.preview])))
const speed = ref(1)
const adjusting = ref(false)
let stop = () => {}
let stopAssistant = () => {}
const assistantOff: (() => void)[] = []
let assistantDisposed = false
async function initAssistant() {
  for (const [name, handler] of [
    ['assistant:open', (value: unknown) => { assignedPet.value = value as number | null; page.value = 'assistant' }],
    ['assistant:drop', (value: unknown) => { droppedFiles.value = value as string[]; page.value = 'assistant' }],
  ] as const) {
    const off = await listen(name, event => handler(event.payload))
    if (assistantDisposed) off(); else assistantOff.push(off)
  }
  if (assistantDisposed) return
  const stop = await startTasks()
  if (assistantDisposed) stop(); else stopAssistant = stop
}

const titles: Record<Page, string> = {
  assistant: 'Your assistant', companion: 'Your companions', packs: 'Pet packs', desktop: 'Desktop view',
  lab: 'Pet Lab', awareness: 'Your day', about: 'Quiet by design',
}

function petActivity(state: string) {
  const labels: Record<string, string> = { relocate: 'Finding a perch', rest: 'Resting', perch: 'Perched', forage: 'Exploring', idle: 'Relaxing', sleep: 'Sleeping', move: 'On the move' }
  return labels[state] ?? state.replace(/_/g, ' ')
}

const status = computed(() => {
  if (!native) return 'Browser preview'
  if (!snapshot.value) return 'Connecting…'
  if (snapshot.value.suspended) return 'Out of the way for a fullscreen app'
  if (snapshot.value.settings.hidden) return 'Taking a little break'
  if (snapshot.value.settings.paused) return 'Resting here'
  if (!pets.value.length) return 'Nobody is out yet'
  const pet = pets.value[0]
  return `${pet.name} · ${petActivity(pet.state)}`
})

onMounted(() => {
  stop = start()
  if (native) void initAssistant().catch(() => {})
})
onUnmounted(() => { stop(); assistantDisposed = true; stopAssistant(); assistantOff.forEach(off => off()) })

watch(() => settings.value?.speedScale, value => {
  if (!adjusting.value && value !== undefined) speed.value = value
}, { immediate: true })

function saveSpeed() {
  adjusting.value = false
  if (settings.value) void saveSettings({ ...settings.value, speedScale: Number(speed.value) })
}
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <a class="brand" href="#" @click.prevent="page = 'companion'"><span class="brand-mark"><Bird :size="25" :stroke-width="1.5" /></span>perch<span class="brand-dot">.</span></a>
      <div class="workspace-label">WORKSPACE</div>
      <nav aria-label="Main navigation">
        <button :class="{ selected: page === 'assistant' }" :aria-current="page === 'assistant' ? 'page' : undefined" @click="page = 'assistant'"><MoveUpRight :size="18" /> Assistant</button>
        <button :class="{ selected: page === 'companion' }" :aria-current="page === 'companion' ? 'page' : undefined" @click="page = 'companion'"><Bird :size="18" /> Companions</button>
        <button :class="{ selected: page === 'packs' }" :aria-current="page === 'packs' ? 'page' : undefined" @click="page = 'packs'"><Package :size="18" /> Pet packs</button>
        <div class="nav-group-label">PREFERENCES</div>
        <button :class="{ selected: page === 'desktop' }" :aria-current="page === 'desktop' ? 'page' : undefined" @click="page = 'desktop'"><Monitor :size="18" /> Desktop view</button>
        <button :class="{ selected: page === 'awareness' }" :aria-current="page === 'awareness' ? 'page' : undefined" @click="page = 'awareness'"><Activity :size="18" /> Your day</button>
        <button :class="{ selected: page === 'lab' }" :aria-current="page === 'lab' ? 'page' : undefined" @click="page = 'lab'"><FlaskConical :size="18" /> Pet Lab</button>
        <button :class="{ selected: page === 'about' }" :aria-current="page === 'about' ? 'page' : undefined" @click="page = 'about'"><ShieldCheck :size="18" /> Privacy</button>
      </nav>
      <div class="sidebar-bottom">
        <span class="sidebar-status"><span class="live-dot" /> {{ pets.length }} companions on desktop</span>
        <span class="version-label">Perch <span>Preview 0.1</span></span>
      </div>
    </aside>

    <main ref="content" class="content">
      <header class="topbar">
        <span>{{ titles[page] }}</span>
        <span class="local-badge"><span /> Private by default.</span>
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
            <div class="eyebrow">YOUR DESKTOP, A LITTLE LIVELIER.</div>
            <h1>Your companions</h1>
            <p>Manage your pets and make them feel at home.</p>
          </div>
          <button class="primary-button" @click="page = 'packs'"><Package :size="16" /> Add companion</button>
        </section>

        <div class="companion-grid">
          <section class="pet-card">
            <div class="card-overline">
              <span class="soft-pill"><span /> {{ pets.length ? 'ON YOUR DESKTOP' : 'NOBODY OUT YET' }}</span>
              <span class="card-number">{{ String(pets.length).padStart(2, '0') }} / {{ String(snapshot?.packs.length ?? 0).padStart(2, '0') }}</span>
            </div>
            <CollectionToolbar v-model:query="petQuery" v-model:filter="petClass" label="Find a companion" :count="visiblePets.length" :options="petFilters" />
            <ul class="pet-list">
              <li v-for="pet in visiblePets" :key="pet.id">
                <div class="pet-avatar"><img v-if="previews.get(pet.packId)" :src="previews.get(pet.packId)!" alt=""><Bird v-else :size="26" /></div>
                <div class="pet-info">
                  <div class="pet-name">{{ pet.name }} <span>{{ pet.class }}</span></div>
                  <p>{{ petActivity(pet.state) }}<span v-if="pet.surface"> · on the {{ pet.surface.replace(/_/g, ' ') }}</span></p>
                </div>
                <button class="pet-ask" :aria-label="'Ask ' + pet.name" @click="assignedPet = pet.id; page = 'assistant'">Ask <ArrowUpRight :size="14" /></button>
                <div class="pet-bars">
                  <label>Energy <span>{{ Math.round(pet.energy * 100) }}%</span><i><b :style="{ width: `${pet.energy * 100}%` }" /></i></label>
                  <label>Mood <span>{{ Math.round(pet.mood * 100) }}%</span><i><b :style="{ width: `${pet.mood * 100}%` }" /></i></label>
                </div>
              </li>
              <li v-if="pets.length && !visiblePets.length" class="pet-empty"><p>No companions match. Try another name or clear the filters.</p></li>
              <li v-if="native && !snapshot" class="pet-empty" role="status"><p>Connecting to your desktop…</p></li>
              <li v-else-if="!pets.length" class="pet-empty">
                <p>Open <button class="text-link" @click="page = 'packs'">Pet packs <ArrowUpRight :size="12" /></button> and bring one out.</p>
              </li>
            </ul>
            <div class="pet-status" aria-live="polite"><span :class="{ sleeping: settings?.paused || settings?.hidden }" />{{ status }}</div>
          </section>

          <section class="controls-card companion-controls" :aria-busy="busy">
            <div class="section-label"><SlidersHorizontal :size="16" /> Desktop controls</div>
            <h2>Set the pace</h2>
            <p class="control-intro">They will find their own way between your windows.</p>
            <button class="primary-button" :disabled="!snapshot || busy" @click="action('relocate')"><MoveUpRight :size="18" /> Find a new perch</button>
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

          </section>
        </div>

        <section class="tips-row">
          <div class="tip">
            <span class="tip-icon"><MousePointer2 :size="20" :stroke-width="1.4" /></span>
            <div><h3>Say a little hello</h3><p>Drag into a Finder folder.<br>Right-click for a task bubble.</p></div>
          </div>
          <div class="tip">
            <span class="tip-icon"><ScanLine :size="20" :stroke-width="1.4" /></span>
            <div><h3>Your windows, their world</h3><p>{{ snapshot?.perchCount ?? 0 }} visible window edges right now.<br><button class="text-link" @click="page = 'desktop'">Take a look <ArrowUpRight :size="12" /></button></p></div>
          </div>
        </section>
      </template>

      <div v-if="assistantVisited" v-show="page === 'assistant'"><AssistantPanel /></div>
      <PacksPanel v-if="page === 'packs'" />
      <DesktopPanel v-else-if="page === 'desktop'" />
      <AwarenessPanel v-else-if="page === 'awareness'" />
      <LabPanel v-else-if="page === 'lab'" />
      <PrivacyPanel v-else-if="page === 'about'" />


    </main>
  </div>
</template>

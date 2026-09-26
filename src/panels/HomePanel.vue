<script setup lang="ts">
import { computed } from 'vue'
import { Bird, Eye, EyeOff, Heart, MessageCircle, Pause, Play, Plus, Shuffle, Zap } from '@lucide/vue'
import { action, busy, call, native, packs, pets, saveSettings, settings, snapshot } from '../state'

const emit = defineEmits<{ go: [page: 'pets' | 'assistant']; ask: [petId: number] }>()

const previews = computed(() => new Map(packs.value.map(pack => [pack.summary.id, pack.summary.preview])))

const greeting = computed(() => {
  const hour = new Date().getHours()
  return hour < 5 ? 'Up late' : hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening'
})

function firstName(name: string) {
  return name.split(' ')[0] ?? name
}

const summary = computed(() => {
  if (native && !snapshot.value) return 'Checking in on your desktop…'
  if (snapshot.value?.suspended) return 'Your pets stepped aside while a fullscreen app is open.'
  if (settings.value?.hidden) return 'Your pets are hidden for now.'
  if (settings.value?.paused) return 'Your pets are resting where they are.'
  const names = pets.value.map(pet => firstName(pet.name))
  if (!names.length) return 'Nobody is out yet. Pick a pet to keep you company.'
  if (names.length === 1) return `${names[0]} is out exploring your desktop.`
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1)} are out exploring your desktop.`
})

const activities: Record<string, string> = {
  relocate: 'Looking for a new spot', rest: 'Resting', perch: 'Perched', forage: 'Exploring',
  idle: 'Relaxing', sleep: 'Sleeping', move: 'On the move',
}
const places: Record<string, string> = {
  window_top: 'on top of a window', window_bottom: 'under a window', window_left: 'on the side of a window',
  window_right: 'on the side of a window', taskbar_top: 'on the taskbar', monitor_bottom: 'at the bottom of the screen',
  monitor_top: 'at the top of the screen', monitor_left: 'on the edge of the screen', monitor_right: 'on the edge of the screen',
}
function activity(state: string, surface: string | null) {
  const doing = activities[state] ?? state.replace(/_/g, ' ')
  const where = surface ? places[surface] : ''
  return where ? `${doing} ${where}` : doing
}

const paces = [
  { label: 'Calm', value: 0.6 },
  { label: 'Normal', value: 1 },
  { label: 'Lively', value: 1.8 },
]
/** The nearest preset, so a speed saved by an older build still shows a choice. */
const pace = computed(() => {
  const current = settings.value?.speedScale ?? 1
  return paces.reduce((best, next) => Math.abs(next.value - current) < Math.abs(best.value - current) ? next : best).value
})
function setPace(value: number) {
  if (settings.value && value !== settings.value.speedScale) void saveSettings({ ...settings.value, speedScale: value })
}
const offline = computed(() => !snapshot.value || busy.value)
/** First-run shortcut: the gallery's best candidates, one click from the desktop. */
const starters = computed(() => packs.value.filter(listing => listing.summary.loadable).slice(0, 3).map(listing => listing.summary))
async function bringOut(packId: string) { await call('spawn_pet', { packId }) }
</script>

<template>
  <section class="home-hero">
    <div>
      <h1>{{ greeting }}</h1>
      <p aria-live="polite">{{ summary }}</p>
    </div>
    <div v-if="pets.length || settings?.paused || settings?.hidden" class="hero-controls">
      <button class="pill-button" :class="{ active: settings?.paused }" :disabled="offline" @click="action('pause')">
        <Play v-if="settings?.paused" :size="16" /><Pause v-else :size="16" />{{ settings?.paused ? 'Resume' : 'Pause' }}
      </button>
      <button class="pill-button" :class="{ active: settings?.hidden }" :disabled="offline" @click="action('visibility')">
        <Eye v-if="settings?.hidden" :size="16" /><EyeOff v-else :size="16" />{{ settings?.hidden ? 'Show' : 'Hide' }}
      </button>
    </div>
  </section>

  <section v-if="!pets.length && starters.length" class="starter">
    <h2>Say hello to someone new</h2>
    <ul class="starter-list">
      <li v-for="pack in starters" :key="pack.id">
        <img v-if="pack.preview" :src="pack.preview" alt="">
        <Bird v-else :size="36" :stroke-width="1.2" />
        <strong>{{ pack.name }}</strong>
        <button class="primary-button" :disabled="busy" @click="bringOut(pack.id)">Bring out</button>
      </li>
    </ul>
    <button class="text-link" @click="emit('go', 'pets')">See all pets</button>
  </section>

  <ul v-else class="pet-cards">
    <li v-for="pet in pets" :key="pet.id" class="pet-tile">
      <div class="pet-portrait">
        <img v-if="previews.get(pet.packId)" :src="previews.get(pet.packId)!" alt="">
        <Bird v-else :size="44" :stroke-width="1.2" />
      </div>
      <div class="pet-tile-body">
        <h2>{{ pet.name }}</h2>
        <p class="pet-doing"><span class="live-dot" :class="{ sleeping: settings?.paused || settings?.hidden || pet.state === 'sleep' }" />{{ activity(pet.state, pet.surface) }}</p>
        <div class="pet-vitals">
          <span :title="`Mood ${Math.round(pet.mood * 100)}%`"><Heart :size="13" /><i><b :style="{ width: `${pet.mood * 100}%` }" /></i><span class="sr-only">Mood {{ Math.round(pet.mood * 100) }}%</span></span>
          <span :title="`Energy ${Math.round(pet.energy * 100)}%`"><Zap :size="13" /><i><b :style="{ width: `${pet.energy * 100}%` }" /></i><span class="sr-only">Energy {{ Math.round(pet.energy * 100) }}%</span></span>
        </div>
      </div>
      <button class="pet-chat" :aria-label="`Ask ${pet.name} for help`" @click="emit('ask', pet.id)"><MessageCircle :size="15" /> Ask for help</button>
    </li>
    <li class="pet-tile add-tile">
      <button @click="emit('go', 'pets')">
        <span class="add-icon"><Plus :size="22" /></span>
        <strong>{{ pets.length ? 'Add another pet' : 'Choose your first pet' }}</strong>
        <span>Browse the pets you can bring out</span>
      </button>
    </li>
  </ul>

  <section v-if="pets.length" class="settings-group home-pace">
    <div class="setting-row">
      <div><h3>Pace</h3><p>How busy your pets are as they wander between windows.</p></div>
      <div class="segmented" role="radiogroup" aria-label="Pace">
        <button v-for="option in paces" :key="option.label" type="button" role="radio" :aria-checked="pace === option.value"
                :disabled="!snapshot" @click="setPace(option.value)">{{ option.label }}</button>
      </div>
    </div>
    <div class="setting-row">
      <div><h3>Fresh scenery</h3><p>Send everyone off to find a new spot.</p></div>
      <button :disabled="offline || !pets.length" @click="action('relocate')"><Shuffle :size="15" /> Move somewhere new</button>
    </div>
  </section>

  <p v-if="pets.length" class="home-tip">Tip: right-click a pet to ask it for help, or drag it onto a folder.</p>
</template>

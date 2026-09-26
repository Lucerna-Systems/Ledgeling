<script setup lang="ts">
import { computed } from 'vue'
import { Activity, Check, ChevronRight, FlaskConical, Monitor, Power, ShieldCheck, Shuffle, X } from '@lucide/vue'
import { action, busy, call, native, pets, saveSettings, settings, snapshot } from '../state'
import { ACCENT_PRESETS, DEFAULT_ACCENT, presetFor } from '../theme'
import type { ScreenView, Settings, ThemeMode } from '../types'

const emit = defineEmits<{ go: [page: 'desktop' | 'awareness' | 'lab'] }>()

const themeChoices: { value: ThemeMode; label: string }[] = [
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'system', label: 'Auto' },
]

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

function save(patch: Partial<Settings>) {
  if (settings.value) void saveSettings({ ...settings.value, ...patch })
}
function toggleAwareness() {
  if (settings.value) save({ awareness: { ...settings.value.awareness, enabled: !settings.value.awareness.enabled } })
}
function toggleReminders() {
  const awareness = settings.value?.awareness
  if (awareness) save({ awareness: { ...awareness, nudge: { ...awareness.nudge, enabled: !awareness.nudge.enabled } } })
}

/** The first reminder sets the ladder; the later tiers keep their spacing. */
const reminderChoices = computed(() => {
  const current = settings.value?.awareness.nudge.tier1Minutes
  const base = [30, 45, 60, 90]
  return current && !base.includes(current) ? [...base, current].sort((a, b) => a - b) : base
})
function setReminder(event: Event) {
  const awareness = settings.value?.awareness
  if (!awareness) return
  const first = Number((event.target as HTMLSelectElement).value)
  const { tier1Minutes, tier2Minutes, tier3Minutes } = awareness.nudge
  save({ awareness: { ...awareness, nudge: {
    ...awareness.nudge,
    tier1Minutes: first,
    tier2Minutes: first + (tier2Minutes - tier1Minutes),
    tier3Minutes: first + (tier3Minutes - tier1Minutes),
  } } })
}

const screens = computed(() => snapshot.value?.screens ?? [])
// A saved pick can name a display that just went away; show the one pets are actually on.
const selectedScreen = computed(() => snapshot.value?.petScreen ?? settings.value?.petScreen ?? '')
function screenLabel(screen: ScreenView, index: number) {
  return screen.isPrimary ? 'Main display' : `Display ${index + 1}`
}
function pickScreen(event: Event) {
  const name = (event.target as HTMLSelectElement).value
  if (name) save({ petScreen: name })
}

const seen = [
  'How long since you last used the keyboard or mouse',
  'What kind of app is in front, such as code, browser or chat',
  'Where your windows are, so pets have places to stand',
  'Where your pointer is, so pets can react to it',
]
const neverSeen = [
  'Window titles, which can hold email subjects and page names',
  'What you type; only that you typed',
  'Your screen. Ledgeling takes no screenshots',
  'Your clipboard, browsing history or open tabs',
]
</script>

<template>
  <section class="page-heading">
    <div>
      <h1>Settings</h1>
      <p>Make Ledgeling feel like yours.</p>
    </div>
  </section>

  <h2 class="group-title">Appearance</h2>
  <section class="settings-group">
    <div class="setting-row">
      <div><h3>Theme</h3><p>Auto follows your system.</p></div>
      <div class="segmented" role="radiogroup" aria-label="Theme">
        <button v-for="choice in themeChoices" :key="choice.value" type="button" role="radio"
                :aria-checked="settings?.theme === choice.value" @click="save({ theme: choice.value })">
          {{ choice.label }}
        </button>
      </div>
    </div>
    <div class="setting-row">
      <div><h3>Accent colour</h3><p>Used for buttons and highlights.</p></div>
      <div class="swatches" role="radiogroup" aria-label="Accent colour">
        <button v-for="preset in ACCENT_PRESETS" :key="preset.id" type="button" class="swatch" role="radio"
                :style="{ background: `linear-gradient(135deg, ${preset.base}, ${preset.glow})` }" :title="preset.label" :aria-label="preset.label"
                :aria-checked="settings?.accent?.toLowerCase() === preset.base" @click="save({ accent: preset.base })" />
        <span class="swatch custom" role="radio" title="Custom" aria-label="Custom colour"
              :aria-checked="!!settings?.accent && !presetFor(settings.accent)">
          <input type="color" :value="settings?.accent ?? DEFAULT_ACCENT" @change="save({ accent: ($event.target as HTMLInputElement).value })" />
        </span>
      </div>
    </div>
  </section>

  <h2 class="group-title">Your pets</h2>
  <section class="settings-group">
    <div class="setting-row">
      <div><h3>Pace</h3><p>How busy your pets are as they wander between windows.</p></div>
      <div class="segmented" role="radiogroup" aria-label="Pace">
        <button v-for="option in paces" :key="option.label" type="button" role="radio" :aria-checked="pace === option.value"
                :disabled="!snapshot" @click="option.value !== settings?.speedScale && save({ speedScale: option.value })">{{ option.label }}</button>
      </div>
    </div>
    <div class="setting-row">
      <div><h3>Fresh scenery</h3><p>Send everyone off to find a new spot.</p></div>
      <button :disabled="!snapshot || busy || !pets.length" @click="action('relocate')"><Shuffle :size="15" /> Move somewhere new</button>
    </div>
    <div v-if="screens.length > 1" class="setting-row">
      <div><h3>Screen</h3><p>Pets live on one screen at a time.</p></div>
      <select :value="selectedScreen" :disabled="busy" aria-label="Screen for pets" @change="pickScreen">
        <option v-for="(screen, index) in screens" :key="screen.name" :value="screen.name">{{ screenLabel(screen, index) }}</option>
      </select>
    </div>
    <div class="setting-row">
      <div><h3>Sounds</h3><p>Let pets make their own little noises.</p></div>
      <button class="toggle" :class="{ enabled: settings?.sounds }" role="switch" :aria-checked="!!settings?.sounds" aria-label="Sounds"
              @click="save({ sounds: !settings?.sounds })"><span /></button>
    </div>
    <div class="setting-row">
      <div><h3>Notice my day</h3><p>Pets get sleepy when you step away and perk up when you return.</p></div>
      <button class="toggle" :class="{ enabled: settings?.awareness.enabled }" role="switch"
              :aria-checked="!!settings?.awareness.enabled" aria-label="Awareness" @click="toggleAwareness"><span /></button>
    </div>
    <div class="setting-row" :class="{ dimmed: !settings?.awareness.enabled }">
      <div><h3>Break reminders</h3><p>After a long stretch, a pet wanders closer. No pop-ups, and never during games or films.</p></div>
      <div class="row-controls">
        <select v-if="settings?.awareness.nudge.enabled" :value="settings.awareness.nudge.tier1Minutes"
                :disabled="!settings.awareness.enabled" aria-label="Remind me after" @change="setReminder">
          <option v-for="minutes in reminderChoices" :key="minutes" :value="minutes">After {{ minutes }} min</option>
        </select>
        <button class="toggle" :class="{ enabled: settings?.awareness.nudge.enabled }" role="switch"
                :aria-checked="!!settings?.awareness.nudge.enabled" aria-label="Break reminders"
                :disabled="!settings?.awareness.enabled" @click="toggleReminders"><span /></button>
      </div>
    </div>
  </section>

  <h2 class="group-title">General</h2>
  <section class="settings-group">
    <div class="setting-row">
      <div><h3>Open at login</h3><p>Start Ledgeling when you log in to your computer.</p></div>
      <button class="toggle" :class="{ enabled: settings?.launchAtLogin }" role="switch" :aria-checked="!!settings?.launchAtLogin" aria-label="Launch at login"
              @click="save({ launchAtLogin: !settings?.launchAtLogin })"><span /></button>
    </div>
    <div class="setting-row">
      <div><h3>Quit Ledgeling</h3><p>Closing this window keeps your pets out. Quitting sends them home.</p></div>
      <button class="danger-button" :disabled="!native" @click="call('quit_app')"><Power :size="15" /> Quit</button>
    </div>
  </section>

  <h2 class="group-title">Privacy</h2>
  <section class="settings-group privacy-summary">
    <div class="privacy-lead">
      <span class="privacy-icon"><ShieldCheck :size="22" :stroke-width="1.6" /></span>
      <div>
        <h3>Everything stays on this computer</h3>
        <p>There is no account and no tracking. Pets only notice what they need to feel alive. The assistant uses an online service only when you pick one and allow it.</p>
      </div>
    </div>
    <div class="privacy-columns">
      <div>
        <h4>What pets notice</h4>
        <ul class="privacy-list"><li v-for="item in seen" :key="item"><Check :size="14" /> {{ item }}</li></ul>
      </div>
      <div>
        <h4>What Ledgeling never looks at</h4>
        <ul class="privacy-list never"><li v-for="item in neverSeen" :key="item"><X :size="14" /> {{ item }}</li></ul>
      </div>
    </div>
  </section>

  <h2 class="group-title">Advanced</h2>
  <section class="settings-group">
    <button class="link-row" @click="emit('go', 'desktop')">
      <Monitor :size="17" /><div><h3>Desktop map</h3><p>See where pets can stand, and pick a screen.</p></div><ChevronRight :size="16" />
    </button>
    <button class="link-row" @click="emit('go', 'awareness')">
      <Activity :size="17" /><div><h3>Awareness details</h3><p>Reminder timing and how apps are categorised.</p></div><ChevronRight :size="16" />
    </button>
    <button class="link-row" @click="emit('go', 'lab')">
      <FlaskConical :size="17" /><div><h3>Pet Lab</h3><p>Tools for people making their own pets.</p></div><ChevronRight :size="16" />
    </button>
  </section>
  <p class="version-note">Ledgeling · Preview 0.1</p>
</template>

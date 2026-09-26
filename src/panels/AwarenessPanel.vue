<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Activity, Clock, Coffee, Sparkles, Plus } from '@lucide/vue'
import { minutes, saveSettings, settings, snapshot } from '../state'
import type { AppCategory } from '../types'

const categories: AppCategory[] = ['code', 'browser', 'comms', 'media', 'creative', 'game', 'other']
const draftName = ref('')
const draftCategory = ref<AppCategory>('code')
const awareness = computed(() => snapshot.value?.awareness ?? null)

/** Every change writes straight through: there is no Save button to forget. */
function update(mutate: (next: NonNullable<typeof settings.value>) => void) {
  if (!settings.value) return
  const next = JSON.parse(JSON.stringify(settings.value))
  mutate(next)
  void saveSettings(next)
}

const tiers = computed(() => settings.value?.awareness.nudge)
const local = ref({ t1: 45, t2: 75, t3: 110 })
watch(tiers, value => {
  if (value) local.value = { t1: value.tier1Minutes, t2: value.tier2Minutes, t3: value.tier3Minutes }
}, { immediate: true })

function commitTiers() {
  update(next => {
    next.awareness.nudge.tier1Minutes = Number(local.value.t1)
    next.awareness.nudge.tier2Minutes = Number(local.value.t2)
    next.awareness.nudge.tier3Minutes = Number(local.value.t3)
  })
}
function addOverride() {
  if (!draftName.value.trim()) return
  update(next => { next.categoryOverrides[draftName.value.trim().toLowerCase()] = draftCategory.value })
  draftName.value = ''
}
function removeOverride(name: string) {
  update(next => { delete next.categoryOverrides[name] })
}
</script>

<template>
  <section class="page-heading">
    <div>
      <div class="eyebrow">OBSERVED, NEVER INTERROGATED</div>
      <h1>Your day</h1>
      <p>Choose how your companions respond to your day.</p>
    </div>
  </section>

  <section class="controls-card">
    <div class="section-label"><Activity :size="16" /> Awareness</div>
    <div class="desktop-details">
      <div>
        <h3>Awareness</h3>
        <p>Let pets respond to activity and breaks. Turn off to stop awareness tracking.</p>
      </div>
      <button class="toggle" :class="{ enabled: settings?.awareness.enabled }" role="switch"
              :aria-checked="!!settings?.awareness.enabled" aria-label="Awareness"
              @click="update(next => { next.awareness.enabled = !next.awareness.enabled })"><span /></button>
    </div>
  </section>

  <div class="awareness-stats" v-if="awareness">
    <div><Clock :size="18" /><strong>{{ awareness.session.idleState.replace('_', ' ') }}</strong><span>{{ Math.round(awareness.session.idleSeconds) }}s since input</span></div>
    <div><Coffee :size="18" /><strong>{{ minutes(awareness.session.stretchMinutes) }}</strong><span>unbroken stretch · {{ awareness.session.breaks }} breaks, {{ awareness.session.microBreaks }} micro-breaks</span></div>
    <div><Sparkles :size="18" /><strong>{{ awareness.category }}</strong><span>foreground category</span></div>
    <div><Activity :size="18" /><strong>energy {{ Math.round(awareness.energy * 100) }}% · mood {{ Math.round(awareness.mood * 100) }}%</strong><span>nudge tier {{ awareness.nudgeTier }} of 3</span></div>
  </div>

  <section class="controls-card">
    <div class="section-label"><Coffee :size="16" /> The nudge ladder</div>
    <p class="control-intro">
      Gentle movement reminds you to take a break. Always quiet during games and films.
    </p>
    <div class="desktop-details">
      <div><h3>Nudges</h3><p>Turn the whole ladder off and keep the rest of the awareness.</p></div>
      <button class="toggle" :class="{ enabled: settings?.awareness.nudge.enabled }" role="switch"
              :aria-checked="!!settings?.awareness.nudge.enabled" aria-label="Nudges"
              @click="update(next => { next.awareness.nudge.enabled = !next.awareness.nudge.enabled })"><span /></button>
    </div>
    <div class="tier-row">
      <label>Restless after <input v-model.number="local.t1" type="number" min="10" max="600" @change="commitTiers"> min</label>
      <label>Closer after <input v-model.number="local.t2" type="number" min="10" max="600" @change="commitTiers"> min</label>
      <label>Alongside after <input v-model.number="local.t3" type="number" min="10" max="600" @change="commitTiers"> min</label>
    </div>
  </section>

  <section class="controls-card">
    <div class="section-label"><Sparkles :size="16" /> Application categories</div>
    <p class="control-intro">
      Adjust how an app is categorised. Ledgeling uses its process name, never its window title.
    </p>
    <ul class="override-list">
      <li v-for="(category, name) in settings?.categoryOverrides ?? {}" :key="name">
        <code>{{ name }}</code> → {{ category }}
        <button class="text-link" @click="removeOverride(String(name))">remove</button>
      </li>
      <li v-if="!Object.keys(settings?.categoryOverrides ?? {}).length" class="muted">Default categories are in use. Add an override below.</li>
    </ul>
    <div class="new-pack-row">
      <input v-model="draftName" placeholder="process name, e.g. helix" aria-label="Process name">
      <select v-model="draftCategory" aria-label="Category">
        <option v-for="option in categories" :key="option" :value="option">{{ option }}</option>
      </select>
      <button class="primary-button" @click="addOverride"><Plus :size="16" /> Add</button>
    </div>
  </section>
</template>

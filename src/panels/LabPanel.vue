<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { FlaskConical, FolderOpen, Plus, Zap, Eye, RefreshCw } from '@lucide/vue'
import { call, packs, pets, refreshPacks, settings, snapshot, saveSettings, validate } from '../state'
import CollectionToolbar from '../components/CollectionToolbar.vue'
import DiagnosticList from './DiagnosticList.vue'
import type { Report } from '../types'

const scenarioQuery = ref('')
const visibleScenarios = computed(() => scenarios.filter(name => name.replace(/_/g, ' ').includes(scenarioQuery.value.trim().toLowerCase())))
const report = ref<Report | null>(null)
const newName = ref('My New Pet')
const newClass = ref('flier')
const created = ref('')
/** A placeholder pack folder, for authors who draw in their own tools and edit pet.json by hand. */
async function scaffold() {
  const path = await call<string>('new_pack', { class: newClass.value, name: newName.value })
  if (path) { created.value = path; await refreshPacks() }
}
const checking = ref('')

/** Scenario buttons: fire an awareness event now instead of waiting an hour. */
const scenarios = [
  'surface_lost', 'surface_moved_fast', 'grabbed', 'released', 'clicked', 'cursor_nearby',
  'user_idle_long', 'user_returned', 'long_session', 'break_taken', 'app_category_changed',
  'hour_changed', 'startup', 'low_energy', 'thrown', 'landed_hard', 'petted', 'pet_nearby',
  'new_day', 'landed', 'edge_reached',
]

onMounted(() => { void call('set_lab', { active: true }); void refreshPacks() })
onUnmounted(() => { void call('set_lab', { active: false }) })

async function check(path: string) {
  checking.value = path
  report.value = await validate(path)
}
async function fire(name: string) { await call('fire_event', { name }) }
function toggleDebug() {
  if (!settings.value) return
  void saveSettings({ ...settings.value, debug: !settings.value.debug })
}
</script>

<template>
  <section class="page-heading">
    <div>
      <div class="eyebrow">FOR PACK AUTHORS</div>
      <h1>Pet Lab<span>.</span></h1>
      <p>Save a file; the pet changes. No restart, no rebuild.</p>
    </div>
  </section>

  <div class="lab-grid">
    <section class="controls-card">
      <div class="section-label"><Eye :size="16" /> Debug overlay</div>
      <p class="control-intro">Draw the surface graph, jump edges, the current path and each pet's state directly on the desktop.</p>
      <button class="toggle" :class="{ enabled: settings?.debug }" role="switch" :aria-checked="!!settings?.debug" aria-label="Debug overlay" @click="toggleDebug"><span /></button>
      <p class="path-note"><RefreshCw :size="13" /> Watching {{ snapshot?.packsDir }} — edits appear within about 200 ms.</p>
      <div class="lab-live">
        <p v-for="pet in pets" :key="pet.id">
          <strong>{{ pet.name }}</strong> · {{ pet.state }} · {{ pet.clip }} · {{ pet.motion.kind }}
          <span v-if="pet.surface">on {{ pet.surface }}</span>
          · energy {{ Math.round(pet.energy * 100) }}% · mood {{ Math.round(pet.mood * 100) }}%
        </p>
        <p v-if="!pets.length">No pets are out. Bring one out from the Pets page.</p>
      </div>
    </section>

    <section class="controls-card">
      <div class="section-label"><Zap :size="16" /> Scenarios</div>
      <p class="control-intro">Force an event so you can see the reaction without arranging the conditions.</p>
      <CollectionToolbar v-model:query="scenarioQuery" label="Find a scenario" :count="visibleScenarios.length" />
      <p v-if="!visibleScenarios.length" class="muted">No matching scenarios.</p>
      <div class="scenario-grid">
        <button v-for="name in visibleScenarios" :key="name" @click="fire(name)">{{ name.replace(/_/g, ' ') }}</button>
      </div>
    </section>
  </div>

  <section class="new-pack">
    <div class="section-label"><FolderOpen :size="16" /> New pack folder</div>
    <p class="control-intro">A working pack with placeholder art, for drawing in your own tools and editing <code>pet.json</code> by hand. To draw inside Ledgeling instead, use <strong>Pets → Make your own pet</strong>.</p>
    <div class="new-pack-row">
      <input v-model="newName" aria-label="Pack name" placeholder="Name">
      <select v-model="newClass" aria-label="How it moves">
        <option value="flier">Flies</option><option value="climber">Climbs</option><option value="walker">Walks</option><option value="hopper">Hops</option>
      </select>
      <button class="primary-button" @click="scaffold"><Plus :size="16" /> New pack</button>
    </div>
    <p v-if="created" class="path-note"><FolderOpen :size="13" /> {{ created }}</p>
  </section>

  <section class="new-pack">
    <div class="section-label"><FlaskConical :size="16" /> Validator</div>
    <div class="lab-validate">
      <button v-for="listing in packs" :key="listing.summary.id" class="text-link" @click="check(listing.summary.directory)">
        Check {{ listing.summary.name }}
      </button>
    </div>
    <p v-if="checking" class="path-note"><code>{{ checking }}</code></p>
    <DiagnosticList v-if="report" :report="report" />
  </section>
</template>

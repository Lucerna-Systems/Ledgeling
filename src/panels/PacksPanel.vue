<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Package, Plus, Trash2, Download, PawPrint, AlertTriangle, FolderOpen } from '@lucide/vue'
import { call, packs, pets, refreshPacks, snapshot } from '../state'
import CollectionToolbar from '../components/CollectionToolbar.vue'
import DiagnosticList from './DiagnosticList.vue'

const query = ref('')
const filter = ref('all')
const filters = [
  { value: 'all', label: 'All packs' }, { value: 'active', label: 'On desktop' },
  { value: 'available', label: 'Available' }, { value: 'issues', label: 'Needs attention' },
  ...['flier', 'climber', 'walker', 'hopper'].map(value => ({ value, label: value[0]!.toUpperCase() + value.slice(1) })),
]
const filteredPacks = computed(() => packs.value.filter(({ summary: pack }) => {
  const matches = [pack.name, pack.author, pack.description].join(' ').toLowerCase().includes(query.value.trim().toLowerCase())
  const active = spawnedFor.value.has(pack.id)
  return matches && (filter.value === 'all' || filter.value === pack.class ||
    (filter.value === 'active' && active) || (filter.value === 'available' && !active && pack.loadable) ||
    (filter.value === 'issues' && (!pack.loadable || pack.errors > 0 || pack.warnings > 0)))
}))
const newName = ref('My New Pet')
const newClass = ref('flier')
const created = ref('')
const exported = ref('')
const open = ref<string | null>(null)

const classes = ['flier', 'climber', 'walker', 'hopper']
const spawnedFor = computed(() => new Set(pets.value.map(p => p.packId)))

onMounted(refreshPacks)

async function spawn(id: string) { await call('spawn_pet', { packId: id }) }
async function despawn(id: string) {
  const pet = pets.value.find(p => p.packId === id)
  if (pet) await call('despawn_pet', { petId: pet.id })
}
async function uninstall(id: string) {
  await call('uninstall_pack', { id })
  await refreshPacks()
}
async function scaffold() {
  const path = await call<string>('new_pack', { class: newClass.value, name: newName.value })
  if (path) { created.value = path; await refreshPacks() }
}
async function exportPack(id: string) {
  const path = await call<string>('export_pack', { id })
  if (path) exported.value = path
}
</script>

<template>
  <section class="page-heading">
    <div>
      <div class="eyebrow">DATA, NEVER CODE</div>
      <h1>Pet packs</h1>
      <p>Find a new friend. Make your desktop your own.</p>
    </div>
  </section>

  <CollectionToolbar v-model:query="query" v-model:filter="filter" label="Search pet packs" :count="filteredPacks.length" :options="filters" />
  <div v-if="!filteredPacks.length" class="empty-state"><Package :size="28" /><h3>{{ packs.length ? 'No matching packs' : 'Your collection starts here' }}</h3><p>{{ packs.length ? 'Try a different search or filter.' : 'Drop a pet pack onto this window to install it.' }}</p></div>
  <div class="pack-grid">
    <article v-for="listing in filteredPacks" :key="listing.summary.id" class="pack-card" :class="{ broken: !listing.summary.loadable }">
      <div class="pack-preview">
        <img v-if="listing.summary.preview" :src="listing.summary.preview" :alt="`${listing.summary.name} preview`">
        <span v-else class="pack-preview-empty"><Package :size="30" :stroke-width="1.2" /></span>
      </div>
      <div class="pack-body">
        <header>
          <h3>{{ listing.summary.name }}</h3>
          <span class="pack-class">{{ listing.summary.class || 'unloadable' }}</span>
        </header>
        <p class="pack-meta">{{ listing.summary.author }} · v{{ listing.summary.version }}<span v-if="listing.summary.builtin"> · bundled</span></p>
        <p class="pack-description">{{ listing.summary.description }}</p>

        <div class="pack-actions">
          <button v-if="!spawnedFor.has(listing.summary.id)" :disabled="!listing.summary.loadable" @click="spawn(listing.summary.id)"><PawPrint :size="14" /> Bring out</button>
          <button v-else @click="despawn(listing.summary.id)"><PawPrint :size="14" /> Send home</button>
          <details class="pack-more">
            <summary>Details</summary>
            <p class="pack-surfaces">{{ listing.summary.surfaces.map(surface => surface.replace(/_/g, ' ')).join(' · ') }}</p>
            <div class="pack-actions"><button @click="exportPack(listing.summary.id)"><Download :size="14" /> Export</button><button class="danger-button" @click="uninstall(listing.summary.id)"><Trash2 :size="14" /> Remove</button></div>
          </details>
        </div>
        <button v-if="listing.report.diagnostics.length" class="text-link" @click="open = open === listing.summary.id ? null : listing.summary.id">
          <AlertTriangle :size="12" />
          {{ listing.summary.errors }} errors · {{ listing.summary.warnings }} warnings
        </button>
        <DiagnosticList v-if="open === listing.summary.id" :report="listing.report" />
      </div>
    </article>
  </div>

  <details class="new-pack">
    <summary><Plus :size="16" /> Create your own pack</summary>
    <p class="control-intro">Start with a working template and add your own artwork.</p>
    <div class="new-pack-row">
      <input v-model="newName" aria-label="Pack name" placeholder="Name">
      <select v-model="newClass" aria-label="Locomotion class">
        <option v-for="option in classes" :key="option" :value="option">{{ option }}</option>
      </select>
      <button class="primary-button" @click="scaffold"><Plus :size="16" /> New pack</button>
    </div>
    <p v-if="created" class="path-note"><FolderOpen :size="13" /> {{ created }}</p>
    <p v-if="exported" class="path-note"><Download :size="13" /> {{ exported }}</p>
  </details>
  <div class="install-note"><FolderOpen :size="16" /><div>Drop a <code>.petpack</code> or pack folder anywhere in this window to install it.<details v-if="snapshot?.packsDir"><summary>Pack location</summary><p>{{ snapshot.packsDir }}</p></details></div></div>
</template>

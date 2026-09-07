<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Package, Plus, Trash2, Download, PawPrint, AlertTriangle, FolderOpen } from '@lucide/vue'
import { call, packs, pets, refreshPacks, snapshot } from '../state'
import DiagnosticList from './DiagnosticList.vue'

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
      <h1>Pet packs<span>.</span></h1>
      <p>Every animal is a folder: two sprite strips and a little JSON.</p>
    </div>
  </section>

  <div class="pack-grid">
    <article v-for="listing in packs" :key="listing.summary.id" class="pack-card" :class="{ broken: !listing.summary.loadable }">
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
        <p v-if="listing.summary.surfaces.length" class="pack-surfaces">{{ listing.summary.surfaces.join(' · ') }}</p>
        <div class="pack-actions">
          <button v-if="!spawnedFor.has(listing.summary.id)" :disabled="!listing.summary.loadable" @click="spawn(listing.summary.id)"><PawPrint :size="14" /> Bring out</button>
          <button v-else @click="despawn(listing.summary.id)"><PawPrint :size="14" /> Send home</button>
          <button @click="exportPack(listing.summary.id)"><Download :size="14" /> Export</button>
          <button @click="uninstall(listing.summary.id)"><Trash2 :size="14" /> Remove</button>
        </div>
        <button v-if="listing.report.diagnostics.length" class="text-link" @click="open = open === listing.summary.id ? null : listing.summary.id">
          <AlertTriangle :size="12" />
          {{ listing.summary.errors }} errors · {{ listing.summary.warnings }} warnings
        </button>
        <DiagnosticList v-if="open === listing.summary.id" :report="listing.report" />
      </div>
    </article>
  </div>

  <section class="new-pack">
    <h2>Start a new pack</h2>
    <p class="control-intro">You get a folder with placeholder art and a manifest that already runs. Change the drawing first; change the JSON when you want more.</p>
    <div class="new-pack-row">
      <input v-model="newName" aria-label="Pack name" placeholder="Name">
      <select v-model="newClass" aria-label="Locomotion class">
        <option v-for="option in classes" :key="option" :value="option">{{ option }}</option>
      </select>
      <button class="primary-button" @click="scaffold"><Plus :size="16" /> New pack</button>
    </div>
    <p v-if="created" class="path-note"><FolderOpen :size="13" /> {{ created }}</p>
    <p v-if="exported" class="path-note"><Download :size="13" /> {{ exported }}</p>
    <p class="path-note"><FolderOpen :size="13" /> Drop a <code>.petpack</code> or a pack folder onto this window to install it. Packs live in {{ snapshot?.packsDir }}</p>
  </section>
</template>

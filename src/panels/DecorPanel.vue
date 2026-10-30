<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, ref } from 'vue'
import { AppWindow, Brush, Copy, Package, Plus, Sparkles, Trash2 } from '@lucide/vue'
import { call, native, settings, snapshot } from '../state'
import FilterTabs from '../components/FilterTabs.vue'
import { decorFromOpened, type DecorDraft, type OpenedDecor } from '../studio/decorModel'
import type { DecorListing, Snapshot } from '../types'

/** The editor is only loaded once someone edits, like the pet Studio. */
const DecorStudio = defineAsyncComponent(() => import('../studio/DecorStudio.vue'))

const tab = ref<'all' | 'windows'>('all')
const decor = ref<DecorListing[]>([])
const editing = ref<{ initial?: DecorDraft; mode: 'new' | 'copy' | 'in_place' } | null>(null)
const confirming = ref('')

const biomes = computed(() => settings.value?.biomes ?? [])
const open = computed(() => new Set(snapshot.value?.openBiomes ?? []))
const tabs = computed(() => [
  { value: 'all', label: 'All decorations', count: decor.value.length },
  { value: 'windows', label: 'Windows with decor', count: biomes.value.length },
])

const ANIMATION_LABELS: Record<string, string> = { breeze: 'Breeze', bob: 'Bob', spin: 'Spin', twinkle: 'Twinkle', rustle: 'Rustle' }
function animationsOf(pack: DecorListing) {
  return [...new Set(pack.props.flatMap(p => p.animations))].map(a => ANIMATION_LABELS[a] ?? a)
}
function thumb(prop: DecorListing['props'][number]) {
  const scale = Math.min(40 / prop.width, 40 / prop.height, 2)
  return {
    width: `${prop.width * scale}px`,
    height: `${prop.height * scale}px`,
    backgroundImage: `url(${prop.image})`,
    backgroundSize: `${prop.frames * prop.width * scale}px ${prop.height * scale}px`,
  }
}

async function refresh() {
  if (!native) return
  decor.value = (await call<DecorListing[]>('list_decor', {}, false)) ?? []
}
onMounted(refresh)

function startNew() { editing.value = { mode: 'new' } }
async function edit(pack: DecorListing, mode: 'copy' | 'in_place') {
  const opened = await call<OpenedDecor>('studio_open_decor', { id: pack.id }, false)
  if (opened) editing.value = { initial: decorFromOpened(opened, mode), mode }
}
async function remove(pack: DecorListing) {
  // Two clicks, so a stray one never throws away someone's drawings.
  if (confirming.value !== pack.id) { confirming.value = pack.id; setTimeout(() => { if (confirming.value === pack.id) confirming.value = '' }, 3000); return }
  confirming.value = ''
  await call('delete_decor', { id: pack.id }, false)
  await refresh()
}
async function closeEditor() { editing.value = null; await refresh() }

async function decorate(id: number) { await call('decorate_biome', { id }) }
async function decorateAnother() { await call('biome_decorate_start') }
async function removeBiome(id: number) {
  const next = await call<Snapshot>('delete_biome', { id }, false)
  if (next) snapshot.value = next
}
</script>

<template>
  <DecorStudio v-if="editing" :initial="editing.initial" :mode="editing.mode" @saved="refresh" @close="closeEditor" />
  <template v-else>
    <div class="sticky-top">
      <section class="page-heading">
        <div>
          <h1>Decor</h1>
          <p>Trees, flowers and other props for the tops of your windows.</p>
        </div>
      </section>
      <FilterTabs v-model="tab" label="Decor" :options="tabs" />
    </div>

    <template v-if="tab === 'all'">
      <div class="filter-bar">
        <span />
        <button type="button" class="primary-button" @click="startNew"><Brush :size="15" /> Make your own decor</button>
      </div>
      <div v-if="!decor.length" class="empty-state">
        <Package :size="28" :stroke-width="1.4" />
        <h3>No decor yet</h3>
        <p>Make a pack of props, and place them on a window.</p>
      </div>
      <div class="pack-grid">
        <article v-for="pack in decor" :key="pack.id" class="pack-card decor-card">
          <div class="decor-shelf">
            <span v-for="prop in pack.props" :key="prop.name" class="decor-thumb" :title="prop.name" :style="thumb(prop)" />
          </div>
          <div class="pack-body">
            <header><h3>{{ pack.name }}</h3></header>
            <p class="pack-description">
              {{ pack.props.length }} {{ pack.props.length === 1 ? 'prop' : 'props' }} · {{ pack.builtin ? 'comes with Ledgeling' : `by ${pack.author || 'you'}` }}
              <template v-if="pack.usedBy"> · in {{ pack.usedBy }} {{ pack.usedBy === 1 ? 'biome' : 'biomes' }}</template>
            </p>
            <p v-if="animationsOf(pack).length" class="decor-animations"><Sparkles :size="12" /> {{ animationsOf(pack).join(', ') }}</p>
            <div class="pack-actions">
              <template v-if="pack.builtin">
                <button type="button" @click="edit(pack, 'copy')"><Copy :size="14" /> Edit a copy</button>
                <button type="button" class="text-link" title="Change the bundled pack itself" @click="edit(pack, 'in_place')">Edit original</button>
              </template>
              <template v-else>
                <button type="button" @click="edit(pack, 'in_place')"><Brush :size="14" /> Edit</button>
                <button type="button" class="text-link" @click="edit(pack, 'copy')">Copy</button>
                <button type="button" class="text-link danger" :aria-label="`Delete ${pack.name}`" @click="remove(pack)"><Trash2 :size="13" /> {{ confirming === pack.id ? 'Click again to delete' : 'Delete' }}</button>
              </template>
            </div>
          </div>
        </article>
      </div>
    </template>

    <template v-else>
      <div class="filter-bar">
        <span />
        <button type="button" class="primary-button" @click="decorateAnother"><Plus :size="15" /> Decorate a window</button>
      </div>
      <div v-if="!biomes.length" class="empty-state">
        <AppWindow :size="28" :stroke-width="1.4" />
        <h3>No windows decorated yet</h3>
        <p>Decorate a window, and its app keeps the scene on whichever of its windows is in front.</p>
      </div>
      <section v-else class="settings-group">
        <div v-for="biome in biomes" :key="biome.id" class="setting-row biome-row">
          <div>
            <h3>{{ biome.app }}</h3>
            <p>{{ biome.props.length }} {{ biome.props.length === 1 ? 'prop' : 'props' }} · {{ open.has(biome.id) ? 'open now' : 'not open on the pets’ screen' }}</p>
          </div>
          <div class="biome-actions">
            <button type="button" class="text-link" :aria-label="`Remove the ${biome.app} biome`" @click="removeBiome(biome.id)">Remove</button>
            <button type="button" :disabled="!open.has(biome.id)" :title="open.has(biome.id) ? 'Bring it forward and place props' : `Open ${biome.app} first`" @click="decorate(biome.id)">Decorate</button>
          </div>
        </div>
      </section>
    </template>
  </template>
</template>

<style scoped>
.decor-shelf { display: flex; align-items: flex-end; justify-content: center; gap: 6px; min-height: 76px; padding: 14px 12px 10px; border-bottom: 1px solid var(--line); background: linear-gradient(to bottom, transparent 70%, color-mix(in srgb, var(--accent) 6%, transparent)); }
.decor-thumb { display: block; background-repeat: no-repeat; image-rendering: pixelated; }
.decor-animations { display: flex; align-items: center; gap: 5px; color: var(--muted); font-size: 12px; margin: 0 0 8px; }
.biome-actions { display: flex; gap: 10px; align-items: center; }
</style>

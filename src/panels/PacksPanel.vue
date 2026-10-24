<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { AlertTriangle, Brush, Check, Download, FolderOpen, MoreHorizontal, Package, PawPrint, Search, Trash2, X } from '@lucide/vue'
import { call, packs, pets, refreshPacks, saveSettings, settings, snapshot } from '../state'
import type { PackSummary } from '../types'
import DiagnosticList from './DiagnosticList.vue'
import FilterPill from '../components/FilterPill.vue'
import FilterTabs from '../components/FilterTabs.vue'

const emit = defineEmits<{ go: [page: 'studio'] }>()

const query = ref('')
const status = ref('all')
const movement = ref('any')
const author = ref('any')
const source = ref('any')
const tag = ref('any')
const exported = ref('')
const open = ref<string | null>(null)

const classes = [
  { value: 'flier', label: 'Flies' }, { value: 'climber', label: 'Climbs' },
  { value: 'walker', label: 'Walks' }, { value: 'hopper', label: 'Hops' },
]
const moves: Record<string, string> = Object.fromEntries(classes.map(c => [c.value, c.label]))
const spawnedFor = computed(() => new Set(pets.value.map(p => p.packId)))

const needsAttention = (pack: PackSummary) => !pack.loadable || pack.errors > 0 || pack.warnings > 0
const sorted = <T,>(values: T[]) => [...new Set(values)].sort()
const authorOptions = computed(() => [{ value: 'any', label: 'Anyone' }, ...sorted(packs.value.map(p => p.summary.author).filter(Boolean)).map(a => ({ value: a, label: `By ${a}` }))])
const tagOptions = computed(() => [{ value: 'any', label: 'Any tag' }, ...sorted(packs.value.flatMap(p => p.summary.tags)).map(t => ({ value: t, label: t[0]!.toUpperCase() + t.slice(1) }))])
const movementOptions = computed(() => [{ value: 'any', label: 'Any movement' }, ...classes.filter(c => packs.value.some(p => p.summary.class === c.value))])
const sourceOptions = [{ value: 'any', label: 'Anywhere' }, { value: 'builtin', label: 'Comes with Ledgeling' }, { value: 'added', label: 'Added by you' }]

/** Everything except the status tab, so each tab can show how many it would hold. */
const narrowed = computed(() => packs.value.filter(({ summary: pack }) =>
  [pack.name, pack.author, pack.description, ...pack.tags].join(' ').toLowerCase().includes(query.value.trim().toLowerCase()) &&
  (movement.value === 'any' || pack.class === movement.value) &&
  (author.value === 'any' || pack.author === author.value) &&
  (source.value === 'any' || (source.value === 'builtin') === pack.builtin) &&
  (tag.value === 'any' || pack.tags.includes(tag.value))))
const statusTabs = computed(() => {
  const list = narrowed.value.map(l => l.summary)
  const tabs = [
    { value: 'all', label: 'All', count: list.length },
    { value: 'out', label: 'On your desktop', count: list.filter(p => spawnedFor.value.has(p.id)).length },
    { value: 'home', label: 'At home', count: list.filter(p => !spawnedFor.value.has(p.id)).length },
  ]
  const broken = list.filter(needsAttention).length
  return broken || status.value === 'issues' ? [...tabs, { value: 'issues', label: 'Needs attention', count: broken }] : tabs
})
const filteredPacks = computed(() => narrowed.value.filter(({ summary: pack }) =>
  status.value === 'all' || (status.value === 'out' && spawnedFor.value.has(pack.id)) ||
  (status.value === 'home' && !spawnedFor.value.has(pack.id)) || (status.value === 'issues' && needsAttention(pack))))
const filtering = computed(() => !!query.value || status.value !== 'all' || movement.value !== 'any' || author.value !== 'any' || source.value !== 'any' || tag.value !== 'any')
function clearFilters() { query.value = ''; status.value = 'all'; movement.value = 'any'; author.value = 'any'; source.value = 'any'; tag.value = 'any' }

onMounted(refreshPacks)

/** Only the counts that are not zero, so a pack with nothing but notes does not
 *  advertise "0 errors". */
function diagnosticLabel(pack: PackSummary) {
  const parts = [[pack.errors, 'error'], [pack.warnings, 'warning'], [pack.notes, 'note']] as const
  const counted = parts.filter(([count]) => count > 0).map(([count, noun]) => `${count} ${noun}${count === 1 ? '' : 's'}`)
  return counted.length ? counted.join(' · ') : 'Details'
}

const tipVisible = computed(() => !!settings.value && !settings.value.tipsDismissed)
async function dismissTip() {
  if (settings.value) await saveSettings({ ...settings.value, tipsDismissed: true })
}

async function spawn(id: string) { await call('spawn_pet', { packId: id }) }
async function despawn(id: string) {
  const pet = pets.value.find(p => p.packId === id)
  if (pet) await call('despawn_pet', { petId: pet.id })
}
async function uninstall(id: string) {
  await call('uninstall_pack', { id })
  await refreshPacks()
}
async function exportPack(id: string) {
  const path = await call<string>('export_pack', { id })
  if (path) exported.value = path
}
</script>

<template>
  <div v-if="tipVisible" class="tip-card" role="note">
    <p><strong>Meet your pet.</strong> Pick it up and throw it. Rub the pointer back and forth over it to pet it.</p>
    <button class="icon-button" aria-label="Dismiss tip" @click="dismissTip"><X :size="14" /></button>
  </div>
  <div class="sticky-top">
  <section class="page-heading">
    <div>
      <h1>Pets</h1>
      <p>Bring a friend out onto your desktop. You can have a few at once.</p>
    </div>
    <label v-if="packs.length" class="search-field">
      <Search :size="15" />
      <input v-model="query" type="search" aria-label="Search pets" placeholder="Search pets">
      <button v-if="query" class="icon-button" aria-label="Clear search" @click="query = ''"><X :size="14" /></button>
    </label>
  </section>

  <FilterTabs v-if="packs.length" v-model="status" label="Status" :options="statusTabs" />
  <div class="filter-bar">
    <template v-if="packs.length">
      <FilterPill v-if="movementOptions.length > 2" v-model="movement" label="How it moves" :options="movementOptions" />
      <FilterPill v-if="authorOptions.length > 2" v-model="author" label="Made by" :options="authorOptions" />
      <FilterPill v-if="packs.some(p => p.summary.builtin) && packs.some(p => !p.summary.builtin)" v-model="source" label="Source" :options="sourceOptions" />
      <FilterPill v-if="tagOptions.length > 2" v-model="tag" label="Tag" :options="tagOptions" />
      <button v-if="filtering" type="button" class="text-link" aria-label="Clear search and filters" @click="clearFilters">Clear filters</button>
    </template>
    <button type="button" class="primary-button make-pet" @click="emit('go', 'studio')"><Brush :size="15" /> Make your own pet</button>
  </div>
  </div>

  <div v-if="!filteredPacks.length" class="empty-state">
    <Package :size="28" :stroke-width="1.4" />
    <h3>{{ packs.length ? 'No pets match' : 'No pets yet' }}</h3>
    <p>{{ packs.length ? 'Try a different search, or clear the filters.' : 'Drop a .petpack file onto this window to add one.' }}</p>
  </div>

  <div class="pack-grid">
    <article v-for="listing in filteredPacks" :key="listing.summary.id" class="pack-card" :class="{ broken: !listing.summary.loadable, out: spawnedFor.has(listing.summary.id) }">
      <div class="pack-preview">
        <img v-if="listing.summary.preview" :src="listing.summary.preview" :alt="listing.summary.name">
        <Package v-else :size="36" :stroke-width="1.2" />
        <span v-if="spawnedFor.has(listing.summary.id)" class="out-badge"><Check :size="12" /> On your desktop</span>
      </div>
      <div class="pack-body">
        <header>
          <h3>{{ listing.summary.name }}</h3>
          <span v-if="moves[listing.summary.class]" class="pack-class">{{ moves[listing.summary.class] }}</span>
        </header>
        <p class="pack-description">{{ listing.summary.description }}</p>
        <p v-if="!listing.summary.loadable" class="pack-problem"><AlertTriangle :size="13" /> This pet can't be brought out right now.</p>
        <div class="pack-actions">
          <button v-if="!spawnedFor.has(listing.summary.id)" class="primary-button" :disabled="!listing.summary.loadable" @click="spawn(listing.summary.id)"><PawPrint :size="15" /> Bring out</button>
          <button v-else @click="despawn(listing.summary.id)">Send home</button>
          <details class="pack-more">
            <summary :aria-label="`More about ${listing.summary.name}`"><MoreHorizontal :size="16" /></summary>
            <div class="pack-menu">
              <p class="pack-meta">By {{ listing.summary.author }} · version {{ listing.summary.version }}<span v-if="listing.summary.builtin"> · comes with Ledgeling</span></p>
              <ul v-if="listing.summary.assistantClips.length" class="pack-clips">
                <li v-for="clip in listing.summary.assistantClips" :key="clip.slot">
                  <span>{{ clip.when[0]!.toUpperCase() + clip.when.slice(1) }}</span>
                  <template v-if="clip.provided">Plays its own <code>{{ clip.slot }}</code></template>
                  <template v-else>Plays <code>{{ clip.clip }}</code> — this pack has no <code>{{ clip.slot }}</code> animation</template>
                </li>
              </ul>
              <button v-if="listing.report.diagnostics.length" class="text-link" @click="open = open === listing.summary.id ? null : listing.summary.id">
                <AlertTriangle :size="12" /> {{ diagnosticLabel(listing.summary) }}
              </button>
              <div class="pack-actions">
                <button @click="exportPack(listing.summary.id)"><Download :size="14" /> Export</button>
                <button class="danger-button" @click="uninstall(listing.summary.id)"><Trash2 :size="14" /> Remove</button>
              </div>
            </div>
          </details>
        </div>
        <DiagnosticList v-if="open === listing.summary.id" :report="listing.report" />
      </div>
    </article>
  </div>

  <p v-if="exported" class="path-note"><Download :size="13" /> Exported to {{ exported }}</p>

  <p class="install-note"><FolderOpen :size="15" /> Got a pet from a friend? Drop its <code>.petpack</code> file onto this window.</p>
  <details v-if="snapshot?.packsDir" class="packs-location"><summary>Where pets are kept</summary><p>{{ snapshot.packsDir }}</p></details>
</template>

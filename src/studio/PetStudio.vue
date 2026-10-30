<script setup lang="ts">
import { computed, ref } from 'vue'
import { AlertCircle, Check, Download, FlaskConical, Palette, PawPrint, RotateCcw, Workflow } from '@lucide/vue'
import { call, native, packs, refreshPacks } from '../state'
import type { PackSummary } from '../types'
import TestTab from './TestTab.vue'
import BehaviorEditor from './BehaviorEditor.vue'
import SpriteEditor from './SpriteEditor.vue'
import { FRAME, movements, payload, problems, slug, starterDraft, uniqueId } from './model'
import { draft, openInStudio } from './state'
import './studio.css'

const emit = defineEmits<{ go: [page: 'pets'] }>()

const tab = ref<'look' | 'behaviour' | 'test'>('look')
const edits = ref(0)
// Read `edits` so drawing, which Vue cannot see, still re-checks the design.
const issues = computed(() => (edits.value, problems(draft.value)))
const saving = ref(false)
const saved = ref<PackSummary | null>(null)
const exported = ref('')
const templates = computed(() => packs.value.filter(p => p.summary.builtin && p.summary.loadable).map(p => p.summary))

async function startFrom(event: Event) {
  const select = event.target as HTMLSelectElement
  const id = select.value
  select.value = ''
  if (!id) return
  saved.value = null
  exported.value = ''
  await openInStudio(id, true)
}
async function exportSaved() {
  if (!saved.value) return
  const result = await call<{ path: string; sha256: string }>('export_pack', { id: saved.value.id }, false)
  if (result) exported.value = result.path
}

/** The gallery picture: the first idle frame, enlarged on a soft backdrop. */
function previewImage() {
  const size = 256
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = size
  const ctx = canvas.getContext('2d')!
  ctx.fillStyle = '#eeeee4'
  ctx.fillRect(0, 0, size, size)
  const idle = draft.value.clips.idle?.frames[0]
  if (idle) {
    const sprite = document.createElement('canvas')
    sprite.width = sprite.height = FRAME
    sprite.getContext('2d')!.putImageData(new ImageData(new Uint8ClampedArray(idle), FRAME, FRAME), 0, 0)
    ctx.imageSmoothingEnabled = false
    ctx.drawImage(sprite, 32, 16, 192, 192)
  }
  return { width: size, height: size, pixels: ctx.getImageData(0, 0, size, size).data }
}

async function save(bringOut: boolean) {
  if (issues.value.length || saving.value) return
  saving.value = true
  saved.value = null
  exported.value = ''
  try {
    // Only a design that already has an id (opened to edit, or saved before)
    // may replace a pack; a new one gets an id of its own first.
    const replace = !!draft.value.id
    if (!replace) draft.value.id = uniqueId(`com.studio.${slug(draft.value.name)}`)
    const summary = await call<PackSummary>('save_studio_pack', { ...payload(draft.value, previewImage()), replace })
    if (summary) {
      draft.value.id = summary.id
      saved.value = summary
      await refreshPacks()
      if (bringOut) await call('spawn_pet', { packId: summary.id })
    }
  } finally { saving.value = false }
}
/** Two clicks, so a stray one never throws away a drawing. */
const confirming = ref(false)
function startOver() {
  if (!confirming.value) { confirming.value = true; setTimeout(() => { confirming.value = false }, 3000); return }
  confirming.value = false
  draft.value = starterDraft(draft.value.movement)
  saved.value = null
  exported.value = ''
}
</script>

<template>
  <section class="studio">
    <header class="studio-head">
      <div class="studio-identity">
        <input v-model="draft.name" class="studio-name" aria-label="Pet name" placeholder="Name your pet" maxlength="40" spellcheck="false">
        <div class="studio-meta">
          <input v-model="draft.author" aria-label="Made by" placeholder="Made by (optional)" maxlength="40">
          <input v-model="draft.description" class="grow" aria-label="Description" placeholder="One line about your pet (optional)" maxlength="140">
        </div>
      </div>
      <div class="studio-actions">
        <button type="button" class="text-link start-over" :class="{ confirming }" @click="startOver"><RotateCcw :size="14" /> {{ confirming ? 'Click again to start over' : 'New pet' }}</button>
        <select v-if="templates.length" class="studio-templates" aria-label="Start from a bundled pet" @change="startFrom">
          <option value="">Start from…</option>
          <option v-for="t in templates" :key="t.id" :value="t.id">{{ t.name }}</option>
        </select>
        <button type="button" :disabled="!!issues.length || saving || !native" @click="save(false)">Save</button>
        <button type="button" class="primary-button" :disabled="!!issues.length || saving || !native" @click="save(true)"><PawPrint :size="15" /> {{ saving ? 'Saving…' : 'Save & bring out' }}</button>
      </div>
    </header>

    <div class="studio-bar">
      <div class="segmented" role="radiogroup" aria-label="How it moves">
        <button v-for="m in movements" :key="m.value" type="button" role="radio" :aria-checked="draft.movement === m.value" :title="m.blurb" @click="draft.movement = m.value">{{ m.label }}</button>
      </div>
      <div class="studio-tabs" role="tablist" aria-label="Studio">
        <button type="button" role="tab" :aria-selected="tab === 'look'" @click="tab = 'look'"><Palette :size="15" /> Look</button>
        <button type="button" role="tab" :aria-selected="tab === 'behaviour'" @click="tab = 'behaviour'"><Workflow :size="15" /> Behaviour</button>
        <button type="button" role="tab" :aria-selected="tab === 'test'" @click="tab = 'test'"><FlaskConical :size="15" /> Test</button>
      </div>
    </div>

    <div v-if="saved" class="studio-saved" role="status">
      <Check :size="16" /> <span><strong>{{ saved.name }}</strong> is saved and ready in your Pets.</span>
      <button type="button" class="text-link" @click="emit('go', 'pets')">See it in Pets</button>
      <button type="button" class="text-link" @click="tab = 'test'"><FlaskConical :size="13" /> Test it</button>
      <button type="button" class="text-link" @click="exportSaved"><Download :size="13" /> Export .petpack</button>
      <span v-if="exported" class="path-note">Exported to {{ exported }}</span>
    </div>
    <div v-else-if="issues.length" class="studio-issues">
      <AlertCircle :size="15" />
      <span>Before saving: {{ issues[0] }}<template v-if="issues.length > 1"> <em>(+{{ issues.length - 1 }} more)</em></template></span>
    </div>

    <SpriteEditor v-show="tab === 'look'" :draft="draft" @edited="edits++" />
    <BehaviorEditor v-if="tab === 'behaviour'" :draft="draft" />
    <TestTab v-if="tab === 'test'" />
  </section>
</template>

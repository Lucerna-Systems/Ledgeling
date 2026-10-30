<script setup lang="ts">
import { computed, ref } from 'vue'
import { Download, FlaskConical, Palette, PawPrint, RotateCcw, Workflow } from '@lucide/vue'
import { call, native, packs, refreshPacks } from '../state'
import type { PackSummary } from '../types'
import TestTab from './TestTab.vue'
import BehaviorEditor from './BehaviorEditor.vue'
import PetLook from './PetLook.vue'
import StudioShell from './StudioShell.vue'
import { FRAME, movements, payload, problems, starterDraft } from './model'
import { draft, openInStudio } from './state'

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
    const summary = await call<PackSummary>('save_studio_pack', payload(draft.value, previewImage()))
    if (summary) {
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
  <StudioShell v-model:name="draft.name" v-model:author="draft.author" v-model:description="draft.description" name-label="Pet name" name-placeholder="Name your pet"
               author-placeholder="Made by (optional)" description-placeholder="One line about your pet (optional)" :issues="issues" :saved="!!saved">
    <template #actions>
      <button type="button" class="text-link start-over" :class="{ confirming }" @click="startOver"><RotateCcw :size="14" /> {{ confirming ? 'Click again to start over' : 'New pet' }}</button>
      <select v-if="templates.length" class="studio-templates" aria-label="Start from a bundled pet" @change="startFrom">
        <option value="">Start from…</option>
        <option v-for="t in templates" :key="t.id" :value="t.id">{{ t.name }}</option>
      </select>
      <button type="button" :disabled="!!issues.length || saving || !native" @click="save(false)">Save</button>
      <button type="button" class="primary-button" :disabled="!!issues.length || saving || !native" @click="save(true)"><PawPrint :size="15" /> {{ saving ? 'Saving…' : 'Save & bring out' }}</button>
    </template>

    <template #tabs>
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
    </template>

    <template v-if="saved" #saved>
      <span><strong>{{ saved.name }}</strong> is saved and ready in your Pets.</span>
      <button type="button" class="text-link" @click="emit('go', 'pets')">See it in Pets</button>
      <button type="button" class="text-link" @click="tab = 'test'"><FlaskConical :size="13" /> Test it</button>
      <button type="button" class="text-link" @click="exportSaved"><Download :size="13" /> Export .petpack</button>
      <span v-if="exported" class="path-note">Exported to {{ exported }}</span>
    </template>

    <PetLook v-show="tab === 'look'" :draft="draft" @edited="edits++" />
    <BehaviorEditor v-if="tab === 'behaviour'" :draft="draft" />
    <TestTab v-if="tab === 'test'" />
  </StudioShell>
</template>

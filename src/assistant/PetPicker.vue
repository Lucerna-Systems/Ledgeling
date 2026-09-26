<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { Bird, ChevronDown, ChevronLeft, ChevronRight, KeyRound, Sparkles } from '@lucide/vue'
import Dropdown from '../components/Dropdown.vue'
import { packs, pets } from '../state'
import { assignedPet, connections, detection, draftConnection, effectiveConnection, loadDetection, taskCall } from './state'
import { providerLabels, type Connection, type ProviderKind } from './types'

const emit = defineEmits<{ setup: [] }>()

const previews = computed(() => new Map(packs.value.map(pack => [pack.summary.id, pack.summary.preview])))
const choices = computed(() => [
  { id: null as number | null, name: 'Any pet', preview: null as string | null },
  ...pets.value.map(pet => ({ id: pet.id as number | null, name: pet.name, preview: previews.value.get(pet.packId) ?? null })),
])
const selectedName = computed(() => choices.value.find(c => c.id === assignedPet.value)?.name ?? 'Any pet')
const overridden = (id: number | null) => id !== null && !!connections.value.perPet?.[id]

function brief(c: Connection) {
  if (c.kind === 'none') return 'No AI'
  return c.model && !['apple'].includes(c.kind) ? c.model : providerLabels[c.kind]
}
const chip = computed(() => brief(effectiveConnection(assignedPet.value)))

function select(id: number | null) { assignedPet.value = id }

// ---- carousel: the selected pet sits in the centre; the rest fan out beside it ----
const index = computed(() => Math.max(0, choices.value.findIndex(c => c.id === assignedPet.value)))
function step(delta: number) {
  const next = choices.value[index.value + delta]
  if (next) assignedPet.value = next.id
}
/** Position of each card relative to the centre, in card slots. */
function place(i: number) {
  const offset = i - index.value
  const distance = Math.abs(offset)
  return {
    transform: `translateX(calc(-50% + ${offset * 124}px)) scale(${distance === 0 ? 1 : Math.max(0.62, 0.84 - (distance - 1) * 0.1)})`,
    opacity: distance === 0 ? 1 : Math.max(0, 0.75 - (distance - 1) * 0.3),
    zIndex: 10 - distance,
    pointerEvents: distance > 3 ? 'none' as const : undefined,
  }
}
// Trackpad swipes arrive as horizontal wheel events; accumulate so one swipe moves one pet.
let swipe = 0
let swipeReset: ReturnType<typeof setTimeout> | undefined
function wheel(event: WheelEvent) {
  if (Math.abs(event.deltaX) <= Math.abs(event.deltaY)) return
  event.preventDefault()
  swipe += event.deltaX
  clearTimeout(swipeReset)
  swipeReset = setTimeout(() => { swipe = 0 }, 180)
  if (Math.abs(swipe) > 60) { step(swipe > 0 ? 1 : -1); swipe = 0 }
}
function key(event: KeyboardEvent) {
  if (event.key === 'ArrowRight') { event.preventDefault(); step(1) }
  else if (event.key === 'ArrowLeft') { event.preventDefault(); step(-1) }
}
// ---- model popover, opened from the model name under the centred pet ----
const open = ref(false)
const anchor = ref<HTMLElement | null>(null)
function outside(event: PointerEvent) {
  // SearchSelect's list lives inside the anchor, so picks there do not count as outside.
  if (open.value && anchor.value && !anchor.value.contains(event.target as Node)) open.value = false
}
function escape(event: KeyboardEvent) { if (event.key === 'Escape') open.value = false }
watch(assignedPet, () => { open.value = false })
onMounted(() => {
  if (!detection.value && isTauri()) void loadDetection()
  document.addEventListener('pointerdown', outside)
})
onUnmounted(() => document.removeEventListener('pointerdown', outside))

// ---- model bar: edits the default for "Any pet", or that pet's own model ----
const current = computed(() => effectiveConnection(assignedPet.value))
const providerChoice = computed(() => overridden(assignedPet.value) || assignedPet.value === null ? current.value.kind : '')
const defaultLabel = computed(() => `Default · ${providerLabels[connections.value.active.kind]}`)
const providers = computed(() => {
  const d = detection.value
  const kinds: { value: ProviderKind; label: string }[] = [{ value: 'none', label: 'No AI · built-in actions only' }]
  const offer = (kind: ProviderKind, label: string, show = true) => { if (show || current.value.kind === kind) kinds.push({ value: kind, label }) }
  offer('apple', 'Apple Intelligence · on this Mac', !!d?.appleSupported)
  offer('claude', 'Claude Code · dev build', !!d?.claudeCode)
  offer('codex', 'ChatGPT · Codex', !!d?.codexInstalled)
  offer('ollama', 'Ollama · on this computer', !!(d?.ollamaApp || d?.ollamaReachable))
  offer('anthropic', d?.anthropicKey ? 'Anthropic' : 'Anthropic · needs a key')
  offer('openai', d?.openaiKey ? 'OpenAI' : 'OpenAI · needs a key')
  offer('compatible', 'Custom endpoint')
  return kinds
})
const missingKey = computed(() => {
  const d = detection.value
  if (!d) return false
  const kind = current.value.kind
  return (kind === 'openai' && !d.openaiKey) || (kind === 'anthropic' && !d.anthropicKey)
})
const showModel = computed(() => !['none', 'apple'].includes(current.value.kind))

const models = ref<string[]>([])
const loadingModels = ref(false)
// A listing fails quietly (no key yet, provider offline); typing a model id still works.
async function loadModels() {
  models.value = []
  if (!showModel.value || !isTauri()) return
  loadingModels.value = true
  try { models.value = await invoke<string[]>('task_models', { config: current.value }) } catch { /* free text still works */ } finally { loadingModels.value = false }
}
watch(() => [assignedPet.value, current.value.kind], () => { void loadModels() }, { immediate: true })

const saving = ref(false)
const note = ref('')
async function persist(next: Connection | null) {
  const petId = assignedPet.value
  if (petId === null) { if (next) connections.value.active = next }
  else {
    if (!connections.value.perPet) connections.value.perPet = {}
    if (next) connections.value.perPet[petId] = next
    else delete connections.value.perPet[petId]
  }
  saving.value = true
  try { await taskCall('task_save_connections', { config: connections.value }); note.value = 'Saved' } catch { note.value = '' } finally { saving.value = false }
  setTimeout(() => { note.value = '' }, 1600)
}
/** Provider rows as the dropdown shows them; a pet also gets "follow the default" first. */
const providerRows = computed(() => [
  ...(assignedPet.value !== null ? [{ value: '' as ProviderKind | '', label: defaultLabel.value }] : []),
  ...providers.value.map(p => ({ value: p.value as ProviderKind | '', label: p.label })),
])
const providerIndex = computed(() => providerRows.value.findIndex(row => row.value === providerChoice.value))
function pickProvider(index: number) {
  const kind = providerRows.value[index]?.value
  if (kind === undefined || kind === providerChoice.value) return
  // The on-device model has its own terms; they are accepted in the full setup, never implied here.
  if (kind === 'apple' && !connections.value.appleTerms?.acceptedVersion) { emit('setup'); return }
  void persist(kind ? draftConnection(kind, '') : null)
}
/** The saved model stays listed even when the provider's listing leaves it out. */
const modelRows = computed(() => current.value.model && !models.value.includes(current.value.model) ? [current.value.model, ...models.value] : models.value)
const modelIndex = computed(() => modelRows.value.indexOf(current.value.model))
// Custom endpoints often have no listing; then the model id is typed.
const typeModel = computed(() => !loadingModels.value && !models.value.length)
function pickModel(model: string) {
  if (model && model !== current.value.model) void persist({ ...current.value, model })
}
</script>

<template>
  <section class="pet-picker">
    <div class="carousel" @wheel="wheel">
      <button type="button" class="carousel-arrow left" aria-label="Previous pet" :disabled="index === 0" @click="step(-1)"><ChevronLeft :size="16" /></button>
      <div class="carousel-stage" role="radiogroup" aria-label="Which pet" @keydown="key">
        <button v-for="(choice, i) in choices" :key="String(choice.id)" type="button" role="radio" class="pet-choice"
                :aria-checked="assignedPet === choice.id" :tabindex="assignedPet === choice.id ? 0 : -1" :style="place(i)" @click="select(choice.id)">
          <span class="choice-ring"><span class="choice-portrait">
            <img v-if="choice.preview" :src="choice.preview" alt="">
            <Sparkles v-else-if="choice.id === null" :size="28" :stroke-width="1.6" />
            <Bird v-else :size="28" :stroke-width="1.4" />
          </span></span>
          <strong :title="choice.name">{{ choice.name }}</strong>
          <small :class="{ own: overridden(choice.id) }">{{ brief(effectiveConnection(choice.id)) }}</small>
        </button>
      </div>
      <button type="button" class="carousel-arrow right" aria-label="Next pet" :disabled="index === choices.length - 1" @click="step(1)"><ChevronRight :size="16" /></button>
    </div>

    <div ref="anchor" class="model-anchor" @keydown="escape">
      <button type="button" class="model-chip" :class="{ own: overridden(assignedPet), open }" aria-haspopup="dialog" :aria-expanded="open"
              :aria-label="`Change the AI ${selectedName} uses, now ${chip}`" @click="open = !open">
        {{ chip }}<ChevronDown :size="13" />
      </button>
      <span v-if="note" class="model-saved" role="status">{{ note }}</span>
      <div v-if="open" class="model-popover" role="dialog" :aria-label="`AI for ${selectedName}`">
        <p class="popover-title">{{ assignedPet === null ? 'Default AI for your pets' : `${selectedName} uses` }}</p>
        <div class="popover-field"><span>Provider</span>
          <Dropdown class="model-provider" label="AI provider" :options="providerRows.map(row => row.label)" :model-value="providerIndex"
                    :disabled="saving" @update:model-value="pickProvider" />
        </div>
        <div v-if="showModel" class="popover-field"><span>Model</span>
          <input v-if="typeModel" :value="current.model" aria-label="Model" placeholder="Type a model id" spellcheck="false"
                 @change="pickModel(($event.target as HTMLInputElement).value.trim())">
          <Dropdown v-else class="model-list" label="Model" :options="modelRows" :model-value="modelIndex"
                    :placeholder="loadingModels ? 'Listing models…' : 'Choose a model'" :disabled="loadingModels || saving"
                    @update:model-value="pickModel(modelRows[$event]!)" />
        </div>
        <p v-if="assignedPet !== null && !overridden(assignedPet)" class="popover-note">Follows the default until you pick something else.</p>
        <button type="button" class="text-link model-setup" @click="open = false; emit('setup')"><KeyRound :size="13" /> {{ missingKey ? 'Add a key' : 'Keys, sign-ins & apps' }}</button>
      </div>
    </div>
  </section>
</template>

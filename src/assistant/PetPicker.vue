<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, toRaw, watch } from 'vue'
import { Bird, ChevronDown, ChevronLeft, ChevronRight, KeyRound, Sparkles } from '@lucide/vue'
import Dropdown from '../components/Dropdown.vue'
import { packs, pets } from '../state'
import { addedProviders, assignedPet, connections, draftConnection, effectiveConnection, providerReady, taskCall } from './state'
import { providerLabels, type Connection, type Connections, type ProviderKind } from './types'

const emit = defineEmits<{ providers: [] }>()

const previews = computed(() => new Map(packs.value.map(pack => [pack.summary.id, pack.summary.preview])))
const choices = computed(() => [
  { id: null as number | null, name: 'Any pet', preview: null as string | null },
  ...pets.value.map(pet => ({ id: pet.id as number | null, name: pet.name, preview: previews.value.get(pet.packId) ?? null })),
])
const selectedName = computed(() => choices.value.find(c => c.id === assignedPet.value)?.name ?? 'Any pet')
const overridden = (id: number | null) => id !== null && !!connections.value.perPet?.[id]

function brief(c: Connection) {
  return providerLabels[c.kind]
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
// ---- provider popover, opened from the provider name under the centred pet ----
const open = ref(false)
const anchor = ref<HTMLElement | null>(null)
function outside(event: PointerEvent) {
  // SearchSelect's list lives inside the anchor, so picks there do not count as outside.
  if (open.value && anchor.value && !anchor.value.contains(event.target as Node)) open.value = false
}
function escape(event: KeyboardEvent) { if (event.key === 'Escape') open.value = false }
watch(assignedPet, () => { open.value = false })
onMounted(() => {
  document.addEventListener('pointerdown', outside)
})
onUnmounted(() => document.removeEventListener('pointerdown', outside))

// ---- provider switch: edits the default for "Any pet", or that pet's own provider ----
const current = computed(() => effectiveConnection(assignedPet.value))
const providerChoice = computed(() => overridden(assignedPet.value) || assignedPet.value === null ? current.value.kind : '')
const defaultLabel = computed(() => `Default · ${providerLabels[connections.value.active.kind]}`)
/** Only added providers can be picked; anything else is set up on the AI providers page first. */
const providerRows = computed(() => {
  const rows: { value: ProviderKind | ''; label: string }[] = []
  if (assignedPet.value !== null) rows.push({ value: '', label: defaultLabel.value })
  rows.push({ value: 'none', label: 'No AI · built-in actions only' })
  for (const kind of addedProviders.value) rows.push({ value: kind, label: providerLabels[kind] })
  // A choice saved before its provider was added stays visible rather than vanishing.
  if (providerChoice.value && !rows.some(row => row.value === providerChoice.value)) rows.push({ value: providerChoice.value, label: providerLabels[providerChoice.value] })
  return rows
})
const providerIndex = computed(() => providerRows.value.findIndex(row => row.value === providerChoice.value))
const termsDue = computed(() => !providerReady(current.value.kind))

const saving = ref(false)
const note = ref('')
async function pickProvider(index: number) {
  const kind = providerRows.value[index]?.value
  if (kind === undefined || kind === providerChoice.value) return
  const next = structuredClone(toRaw(connections.value))
  const petId = assignedPet.value
  const connection = kind ? draftConnection(kind) : null
  if (petId === null) next.active = connection ?? draftConnection('none')
  else if (connection) next.perPet[petId] = connection
  else delete next.perPet[petId]
  saving.value = true
  // The shared state changes only once the backend has saved, so what the
  // picker shows is always what a task will run on.
  try { connections.value = await taskCall<Connections>('task_save_connections', { config: next }); note.value = 'Saved' } catch { note.value = '' } finally { saving.value = false }
  setTimeout(() => { note.value = '' }, 1600)
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
        <p v-if="!addedProviders.length" class="popover-note">No AI providers added yet. Set one up once, then switch here.</p>
        <p v-else-if="termsDue" class="popover-note warn">{{ providerLabels[current.kind] }} needs its terms accepted before it can run.</p>
        <p v-if="assignedPet !== null && !overridden(assignedPet)" class="popover-note">Follows the default until you pick something else.</p>
        <button type="button" class="text-link model-setup" @click="open = false; emit('providers')"><KeyRound :size="13" /> {{ addedProviders.length ? 'Manage AI providers' : 'Set up an AI provider' }}</button>
      </div>
    </div>
  </section>
</template>

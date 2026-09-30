<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { Sparkles, Undo2 } from '@lucide/vue'
import { effectiveConnection, startTasks, taskCall, tasks } from '../assistant/state'
import { providerLabels, terminal, type Task, type TaskRequest } from '../assistant/types'
import { apply, behaviourPrompt, parseSuggestion, restore, snapshot } from './ai'
import type { Draft } from './model'

const props = defineProps<{ draft: Draft }>()

const request = ref('')
const taskId = ref<number | null>(null)
const error = ref('')
const note = ref('')
const undo = ref<ReturnType<typeof snapshot> | null>(null)

/** The request names no pet, so it runs on the default connection. */
const connection = computed(() => effectiveConnection(null))
const ready = computed(() => connection.value.kind !== 'none')
const provider = computed(() => providerLabels[connection.value.kind] ?? connection.value.kind)
const working = computed(() => taskId.value !== null)
const task = computed(() => tasks.value.find(t => t.id === taskId.value) ?? null)

// The Studio can open before the Assistant page has ever been visited.
let stop = () => {}
void startTasks().then(off => { stop = off }).catch(() => {})
onUnmounted(() => stop())

async function ask() {
  const text = request.value.trim()
  if (!text || working.value || !ready.value) return
  error.value = ''
  note.value = ''
  const body: TaskRequest = {
    prompt: behaviourPrompt(props.draft, text), roots: [], attachments: [], preset: '',
    mode: ['codex', 'claude'].includes(connection.value.kind) ? 'advanced' : 'standard',
    parentId: null, petId: null, allowCloud: false, allowWeb: false,
  }
  try { taskId.value = (await taskCall<Task>('task_start', { request: body })).id } catch (e) { error.value = String(e) }
}

watch(task, t => {
  if (!t || !terminal(t.status)) return
  taskId.value = null
  if (t.status !== 'completed') { error.value = t.error || (t.status === 'cancelled' ? '' : 'The assistant could not finish that.'); return }
  try {
    const suggestion = parseSuggestion(t.result, props.draft)
    undo.value = snapshot(props.draft)
    apply(props.draft, suggestion)
    request.value = ''
    note.value = `Updated your behaviours${suggestion.dropped ? `. ${suggestion.dropped} thing${suggestion.dropped === 1 ? '' : 's'} it suggested didn't fit the pet format and ${suggestion.dropped === 1 ? 'was' : 'were'} left out` : ''}.`
  } catch (e) { error.value = `${(e as Error).message} Try asking again.` }
})

function cancel() { if (taskId.value !== null) void taskCall('task_cancel', { id: taskId.value }).catch(() => {}) }
function revert() { if (undo.value) { restore(props.draft, undo.value); undo.value = null; note.value = 'Put your behaviours back.' } }
</script>

<template>
  <section class="block-section ai-helper">
    <header class="block-section-head">
      <h2><Sparkles :size="16" /> Describe it</h2>
      <p v-if="ready">Say what your pet should do and {{ provider }} drafts the behaviours. Only this description and the list of behaviours below are sent, never your drawings.</p>
      <p v-else>Connect an AI in the Assistant settings to draft behaviours from a description.</p>
    </header>
    <form class="ai-form" @submit.prevent="ask">
      <input v-model="request" :disabled="!ready || working" maxlength="500" aria-label="What should your pet do?" placeholder="e.g. naps when I'm away, greets me when I'm back, hides when I'm gaming">
      <button v-if="!working" class="primary-button" type="submit" :disabled="!ready || !request.trim()">Draft</button>
      <button v-else type="button" @click="cancel">Stop</button>
    </form>
    <p v-if="working" class="block-note" role="status">{{ task?.progress || 'Starting' }}…</p>
    <p v-else-if="error" class="block-note warn" role="alert">{{ error }}</p>
    <p v-else-if="note" class="block-note" role="status">{{ note }} <button v-if="undo" type="button" class="text-link" @click="revert"><Undo2 :size="12" /> Undo</button></p>
  </section>
</template>

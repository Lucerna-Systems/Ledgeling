<script setup lang="ts">
import { computed, ref } from 'vue'
import { CircleDot, LoaderCircle, Tag, UserPlus, X } from '@lucide/vue'
import GhMarkdown from './GhMarkdown.vue'
import Picker from './Picker.vue'
import { labelStyle } from './format'
import { act, composing, follow, gh, loadMeta, meta, repo, show } from './store'

const title = ref('')
const body = ref('')
const preview = ref(false)
const labels = ref<string[]>([])
const assignMe = ref(false)
const picking = ref(false)
const busy = ref(false)

const labelOptions = computed(() => (meta.value?.labels ?? []).map(l => ({ value: l.name, color: l.color, hint: l.description })))
const colorOf = (name: string) => meta.value?.labels.find(l => l.name === name)?.color ?? ''
async function pickLabels() { await loadMeta(); picking.value = true }
function applyLabels(add: string[], remove: string[]) { labels.value = [...labels.value.filter(l => !remove.includes(l)), ...add] }

const cancel = () => { composing.value = null }

async function create() {
  if (!title.value.trim() || busy.value) return
  busy.value = true
  try {
    const url = await act({ action: 'createIssue', title: title.value, body: body.value, labels: labels.value, assignees: assignMe.value ? ['@me'] : [] })
    const number = Number(/\/issues\/(\d+)/.exec(url)?.[1])
    if (number) show('issue', number)
    else composing.value = null
  } catch { /* shown above */ } finally { busy.value = false }
}
</script>

<template>
  <form class="gh-compose" @submit.prevent="create">
    <header class="gh-compose-head">
      <CircleDot :size="15" class="gh-state-icon open" />
      <h1>New issue <span class="muted">in {{ repo?.nameWithOwner }}</span></h1>
      <span class="grow" />
      <button type="button" class="icon" title="Cancel" @click="cancel"><X :size="14" /></button>
    </header>
    <input v-model="title" v-focus class="gh-title-input" placeholder="Title" aria-label="Title" required>
    <div class="gh-composer-tabs">
      <span class="segmented-mini" role="group" aria-label="Description view">
        <button type="button" :aria-pressed="!preview" @click="preview = false">Write</button>
        <button type="button" :aria-pressed="preview" @click="preview = true">Preview</button>
      </span>
    </div>
    <GhMarkdown v-if="preview" class="gh-preview tall" :text="body" :repo-url="repo?.url" @link="follow" />
    <textarea v-else v-model="body" class="gh-textarea tall" placeholder="What happened, and what did you expect? Markdown works here." aria-label="Description" @keydown.meta.enter.prevent="create" @keydown.ctrl.enter.prevent="create" />
    <div class="gh-compose-meta">
      <button type="button" class="ghost" @click="pickLabels"><Tag :size="13" /> Labels</button>
      <span v-for="l in labels" :key="l" class="gh-label" :style="labelStyle(colorOf(l))">{{ l }}</span>
      <label v-if="gh?.account" class="gh-check-label"><input v-model="assignMe" type="checkbox"><UserPlus :size="12" /> Assign to me</label>
    </div>
    <div class="gh-composer-actions">
      <span class="grow" />
      <button type="button" class="ghost" @click="cancel">Cancel</button>
      <button type="submit" class="primary-button" :disabled="!title.trim() || busy"><LoaderCircle v-if="busy" :size="13" class="spin" /> Create issue</button>
    </div>
    <Picker v-if="picking" title="Labels" :options="labelOptions" :selected="labels" @apply="applyLabels" @close="picking = false" />
  </form>
</template>

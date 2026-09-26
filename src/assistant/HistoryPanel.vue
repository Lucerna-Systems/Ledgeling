<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { History, Search, X } from '@lucide/vue'
import { pets } from '../state'
import { tasks, taskError } from './state'
import { providerLabels, terminal, type Task } from './types'
import FilterPill from '../components/FilterPill.vue'
import FilterTabs from '../components/FilterTabs.vue'
import TaskCard from './TaskCard.vue'
import './assistant.css'

const emit = defineEmits<{ followup: [task: Task] }>()

const query = ref('')
const status = ref('all')
const when = ref('any')
const pet = ref('any')
const kind = ref('any')
const provider = ref('any')
const order = ref('newest')
const limit = ref(10)

type Bucket = 'active' | 'waiting' | 'done' | 'stopped'
function bucket(task: Task): Bucket {
  if (task.status === 'waiting') return 'waiting'
  if (!terminal(task.status)) return 'active'
  return ['completed', 'undone'].includes(task.status) ? 'done' : 'stopped'
}
function kindOf(task: Task) {
  return task.request.preset ? 'builtin' : task.request.mode === 'advanced' ? 'agent' : 'ai'
}

const DAY = 86400
function startOfToday() {
  const now = new Date()
  return new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000
}
const ranges: Record<string, number> = { today: 0, week: 6, month: 29 }

const petName = (id: number) => pets.value.find(p => p.id === id)?.name ?? `Pet ${id}`
const petOptions = computed(() => {
  const ids = [...new Set(tasks.value.map(t => t.request.petId).filter((id): id is number => id !== null))]
  return [{ value: 'any', label: 'Any pet' }, ...ids.map(id => ({ value: String(id), label: petName(id) }))]
})
const providerOptions = computed(() => {
  const kinds = [...new Set(tasks.value.filter(t => !t.request.preset).map(t => t.connection.kind))]
  return [{ value: 'any', label: 'Any AI' }, ...kinds.map(k => ({ value: k, label: providerLabels[k] ?? k }))]
})

/** Everything except the status tab, so each tab can show how many it would hold. */
const narrowed = computed(() => {
  const text = query.value.trim().toLowerCase()
  const since = when.value === 'any' ? -Infinity : startOfToday() - ranges[when.value]! * DAY
  return tasks.value.filter(task =>
    task.createdAt >= since &&
    (pet.value === 'any' || String(task.request.petId) === pet.value) &&
    (kind.value === 'any' || kindOf(task) === kind.value) &&
    (provider.value === 'any' || (!task.request.preset && task.connection.kind === provider.value)) &&
    (!text || [task.request.prompt, task.result].join(' ').toLowerCase().includes(text)))
})
const counts = computed(() => {
  const all = { all: narrowed.value.length, active: 0, waiting: 0, done: 0, stopped: 0 }
  for (const task of narrowed.value) all[bucket(task)]++
  return all
})
const statusTabs = computed(() => [
  { value: 'all', label: 'All', count: counts.value.all },
  { value: 'waiting', label: 'Needs you', count: counts.value.waiting },
  { value: 'active', label: 'In progress', count: counts.value.active },
  { value: 'done', label: 'Done', count: counts.value.done },
  { value: 'stopped', label: "Didn't finish", count: counts.value.stopped },
])
const filtered = computed(() => narrowed.value
  .filter(task => status.value === 'all' || bucket(task) === status.value)
  .sort((a, b) => order.value === 'newest' ? b.id - a.id : a.id - b.id))

function dayLabel(seconds: number) {
  const today = startOfToday()
  if (seconds >= today) return 'Today'
  if (seconds >= today - DAY) return 'Yesterday'
  const date = new Date(seconds * 1000)
  if (seconds >= today - 6 * DAY) return date.toLocaleDateString(undefined, { weekday: 'long' })
  return date.toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: date.getFullYear() === new Date().getFullYear() ? undefined : 'numeric' })
}
const groups = computed(() => {
  const out: { label: string; tasks: Task[] }[] = []
  for (const task of filtered.value.slice(0, limit.value)) {
    const label = dayLabel(task.createdAt)
    if (out.at(-1)?.label === label) out.at(-1)!.tasks.push(task)
    else out.push({ label, tasks: [task] })
  }
  return out
})

const filtering = computed(() => !!query.value || status.value !== 'all' || when.value !== 'any' || pet.value !== 'any' || kind.value !== 'any' || provider.value !== 'any')
function clear() { query.value = ''; status.value = 'all'; when.value = 'any'; pet.value = 'any'; kind.value = 'any'; provider.value = 'any' }
watch([query, status, when, pet, kind, provider, order], () => { limit.value = 10 })
</script>

<template>
  <section class="page-heading">
    <div><h1>History</h1><p>Everything you've asked for, and what came of it.</p></div>
    <label v-if="tasks.length" class="search-field">
      <Search :size="15" />
      <input v-model="query" type="search" aria-label="Search history" placeholder="Search requests and replies">
      <button v-if="query" class="icon-button" aria-label="Clear search" @click="query = ''"><X :size="14" /></button>
    </label>
  </section>
  <div v-if="taskError" class="error-banner" role="alert">{{ taskError }}<button aria-label="Dismiss task error" @click="taskError = ''"><X :size="15" /></button></div>

  <template v-if="tasks.length">
    <FilterTabs v-model="status" label="Status" :options="statusTabs" />
    <div class="filter-bar">
      <FilterPill v-model="when" label="When" :options="[{ value: 'any', label: 'Any time' }, { value: 'today', label: 'Today' }, { value: 'week', label: 'Past 7 days' }, { value: 'month', label: 'Past 30 days' }]" />
      <FilterPill v-if="petOptions.length > 2" v-model="pet" label="Pet" :options="petOptions" />
      <FilterPill v-model="kind" label="Kind" :options="[{ value: 'any', label: 'Any kind' }, { value: 'builtin', label: 'Built-in actions' }, { value: 'ai', label: 'Ask AI' }, { value: 'agent', label: 'Agent' }]" />
      <FilterPill v-if="providerOptions.length > 2" v-model="provider" label="AI" :options="providerOptions" />
      <button v-if="filtering" type="button" class="text-link" aria-label="Clear search and filters" @click="clear">Clear filters</button>
      <FilterPill v-model="order" class="filter-sort" label="Order" :options="[{ value: 'newest', label: 'Newest first' }, { value: 'oldest', label: 'Oldest first' }]" />
    </div>
  </template>

  <div v-if="!filtered.length" class="empty-state">
    <History :size="28" :stroke-width="1.4" />
    <h3>{{ tasks.length ? 'Nothing matches these filters' : 'Nothing here yet' }}</h3>
    <p>{{ tasks.length ? 'Try a wider date range or clear the filters.' : 'Requests you make in the Assistant will show up here.' }}</p>
  </div>
  <section v-for="group in groups" :key="group.label" class="history-group">
    <h2 class="group-title">{{ group.label }}</h2>
    <TaskCard v-for="task in group.tasks" :key="task.id" :task="task" @followup="emit('followup', $event)" />
  </section>
  <button v-if="filtered.length > limit" class="load-more" @click="limit += 10">Show more · {{ filtered.length - limit }} left</button>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import CollectionToolbar from '../components/CollectionToolbar.vue'
import { Search, Sparkles, FolderPlus, Paperclip, Settings2, ArrowUpRight } from '@lucide/vue'
import { pets } from '../state'
import { tasks, taskError, assignedPet, droppedFiles, connections, effectiveConnection, taskCall } from './state'
import { terminal, type Task, type TaskRequest } from './types'
import TaskCard from './TaskCard.vue'
import ConnectionsPanel from './ConnectionsPanel.vue'
import './assistant.css'
const historyQuery = ref('')
const historyFilter = ref('all')
const historyLimit = ref(10)
const historyFilters = [
  { value: 'all', label: 'All tasks' }, { value: 'active', label: 'In progress' },
  { value: 'waiting', label: 'Needs approval' }, { value: 'completed', label: 'Completed' },
  { value: 'failed', label: 'Failed' }, { value: 'cancelled', label: 'Cancelled' },
]
const filteredTasks = computed(() => tasks.value.filter(task =>
  (historyFilter.value === 'all' || task.status === historyFilter.value || (historyFilter.value === 'active' && !terminal(task.status))) &&
  [task.request.prompt, task.result, task.connection.kind].join(' ').toLowerCase().includes(historyQuery.value.trim().toLowerCase())
).sort((a, b) => b.id - a.id))
watch([historyQuery, historyFilter], () => { historyLimit.value = 10 })
const prompt = ref('')
const roots = ref<string[]>([])
const attachments = ref<string[]>([])
const preset = ref('')
// Every gate follows the connection the chosen companion would actually
// run on — a per-pet override changes the mode and consent rules too.
const effective = computed(() => effectiveConnection(assignedPet.value))
const override = computed(() => assignedPet.value !== null && !!connections.value.perPet?.[assignedPet.value])
const mode = computed(() => !preset.value && ['codex', 'claude'].includes(effective.value.kind) ? 'advanced' : 'standard')
const needsConnection = computed(() => !preset.value && effective.value.kind === 'none')
const parent = ref<number | null>(null)
const allowCloud = ref(false)
const allowWeb = ref(false)
const sending = ref(false)
const setup = ref(false)
const remote = computed(() => !['none', 'ollama'].includes(effective.value.kind) && !preset.value)
const assignedName = computed(() => pets.value.find(p => p.id === assignedPet.value)?.name)
watch(assignedPet, async petId => { if (petId !== null) { try { const folder = await taskCall<string | null>('pet_folder', { petId }); if (folder) roots.value = [folder] } catch { /* shared error */ } } }, { immediate: true })
watch(droppedFiles, paths => { attachments.value = [...new Set([...attachments.value, ...paths])] })
function shortcut(kind: string) {
  if (kind === 'organise') { preset.value = 'sort_type'; prompt.value = 'Organise my folder' }
  else { preset.value = ''; prompt.value = kind === 'summarise' ? 'Summarise the attached document and extract the key points.' : 'Research '; allowWeb.value = kind === 'research' }
}
async function pick(folders: boolean) {
  try { const selected = await taskCall<string[]>('task_pick', { folders }); if (folders) roots.value = [...new Set([...roots.value, ...selected])]; else attachments.value = [...new Set([...attachments.value, ...selected])] } catch { /* shared error */ }
}
async function submit() {
  if (sending.value || needsConnection.value || !prompt.value.trim() || (remote.value && !allowCloud.value)) return
  sending.value = true
  try {
    const request: TaskRequest = { prompt: prompt.value, roots: roots.value, attachments: attachments.value, preset: preset.value, mode: mode.value, parentId: parent.value, petId: assignedPet.value ?? pets.value[0]?.id ?? null, allowCloud: allowCloud.value, allowWeb: allowWeb.value }
    const task = await taskCall<Task>('task_start', { request })
    tasks.value = [task, ...tasks.value.filter(t => t.id !== task.id)]; prompt.value = ''; parent.value = null
  } catch { /* shared error */ } finally { sending.value = false }
}
function followup(task: Task) { parent.value = task.id; preset.value = ''; roots.value = [...task.request.roots]; attachments.value = [...task.request.attachments]; prompt.value = ''; document.getElementById('task-prompt')?.focus() }
async function clearOverride() {
  const petId = assignedPet.value
  if (petId === null || !connections.value.perPet) return
  delete connections.value.perPet[petId]
  try { await taskCall('task_save_connections', { config: connections.value }) } catch { /* shared error */ }
}
</script>
<template>
  <section class="page-heading"><div><h1>Your assistant</h1><p>A little help with the things on your mind.</p></div><span class="connection-badge"><span class="live-dot" />{{ effective.kind === 'none' ? 'Local actions' : effective.kind }}{{ override ? ' · custom model' : '' }}</span></section>
  <div v-if="taskError" class="error-banner" role="alert">{{ taskError }}<button aria-label="Dismiss task error" @click="taskError = ''">×</button></div>
  <section class="task-composer">
    <div class="composer-top"><label>Companion<select v-model="assignedPet"><option :value="null">Any companion</option><option v-for="pet in pets" :key="pet.id" :value="pet.id">{{ pet.name }}</option></select></label><button :aria-expanded="setup" @click="setup = !setup"><Settings2 :size="15" />{{ setup ? 'Close connections' : 'Connections' }}</button></div>
    <ConnectionsPanel v-if="setup" />
    <div class="task-shortcuts"><button @click="shortcut('organise')"><FolderPlus :size="14" /> Organise files</button><button @click="shortcut('summarise')"><Sparkles :size="14" /> Summarise</button><button @click="shortcut('research')"><Search :size="14" /> Research</button></div>
    <form @submit.prevent="submit">
      <p v-if="parent" class="followup-note">Following up on task #{{ parent }} <button type="button" @click="parent = null">Clear</button></p>
      <label for="task-prompt" class="sr-only">Your request</label><textarea id="task-prompt" v-model="prompt" rows="3" maxlength="20000" required placeholder="Sort my folder, explain a document, or find something out…" />
      <details class="request-options"><summary>Task options <span>{{ preset ? 'Built-in action' : mode === 'advanced' ? 'Agent' : 'Ask AI' }}</span></summary>
      <div class="composer-options">
        <label>Action<select v-model="preset"><option value="">Ask AI</option><option value="sort_type">Sort by file type · no AI</option><option value="sort_date">Sort by modified month · no AI</option><option value="rename">Batch rename · request is name prefix</option><option value="find">Find files · request is search text</option><option value="duplicates">Find exact duplicates · no deletion</option></select></label>
      </div>
      <p v-if="mode === 'advanced'" class="task-problem">This connection uses an agent. Review command approvals carefully; arbitrary commands cannot always be undone.</p>
      </details>
      <div class="task-actions attachments-actions"><button type="button" @click="pick(true)"><FolderPlus :size="15" /> Add folder</button><button type="button" @click="pick(false)"><Paperclip :size="15" /> Attach files</button></div>
      <ul class="attachment-list"><li v-for="path in roots" :key="path">Folder · {{ path }} <button type="button" :aria-label="`Remove ${path}`" @click="roots = roots.filter(p => p !== path)">×</button></li><li v-for="path in attachments" :key="path">Document · {{ path }} <button type="button" :aria-label="`Remove ${path}`" @click="attachments = attachments.filter(p => p !== path)">×</button></li></ul>
      <p class="task-meta">{{ preset ? 'Built-in action · runs locally' : `${effective.kind === 'none' ? 'No AI connection' : `${effective.kind}${effective.model ? ` ${effective.model}` : ''}`} · ${remote ? 'selected content is sent to this provider' : 'local processing'}` }}{{ override && assignedName ? ` — ${assignedName} uses its own model ` : '' }}<button v-if="override" class="text-link" @click="clearOverride">Use the default</button></p>
      <label v-if="remote" class="check-label"><input v-model="allowCloud" type="checkbox"> Allow this provider to access selected content</label>
      <label v-if="!preset" class="check-label"><input v-model="allowWeb" type="checkbox"> Allow web research</label>
      <p v-if="needsConnection" class="connection-hint">Choose a built-in action above, or open Connections to add an AI provider.</p>
      <div class="composer-bottom"><span>Changes are previewed before applying.</span><button class="primary-button" :disabled="sending || needsConnection || !prompt.trim() || (remote && !allowCloud)">{{ sending ? 'Starting…' : 'Send request' }}<ArrowUpRight :size="15" /></button></div>
    </form>
  </section>
  <section class="task-history">
    <div class="section-heading"><h2>Task history <span>{{ tasks.length }}</span></h2></div>
    <CollectionToolbar v-model:query="historyQuery" v-model:filter="historyFilter" label="Search tasks" :count="filteredTasks.length" :options="historyFilters" />
    <div v-if="!filteredTasks.length" class="empty-state"><Search :size="26" /><h3>{{ tasks.length ? 'No matching tasks' : 'A fresh start' }}</h3><p>{{ tasks.length ? 'Try a different search or status.' : 'Your requests and results will appear here.' }}</p></div>
    <TaskCard v-for="task in filteredTasks.slice(0, historyLimit)" :key="task.id" :task="task" @followup="followup" />
    <button v-if="filteredTasks.length > historyLimit" class="load-more" @click="historyLimit += 10">Show more tasks · {{ filteredTasks.length - historyLimit }} remaining</button>
  </section>
</template>

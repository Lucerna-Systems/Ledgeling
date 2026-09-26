<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowUp, FolderPlus, Globe, Paperclip, Search, Sparkles, X } from '@lucide/vue'
import { pets } from '../state'
import { tasks, taskError, assignedPet, droppedFiles, effectiveConnection, pendingFollowup, taskCall } from './state'
import { presets, providerLabels, type Task, type TaskRequest } from './types'
import ConnectionsPanel from './ConnectionsPanel.vue'
import PetPicker from './PetPicker.vue'
import './assistant.css'

const prompt = ref('')
const roots = ref<string[]>([])
const attachments = ref<string[]>([])
const preset = ref('')
// Every gate follows the connection the chosen companion would actually
// run on — a per-pet override changes the mode and consent rules too.
const effective = computed(() => effectiveConnection(assignedPet.value))
const mode = computed(() => !preset.value && ['codex', 'claude'].includes(effective.value.kind) ? 'advanced' : 'standard')
const needsConnection = computed(() => !preset.value && effective.value.kind === 'none')
const parent = ref<number | null>(null)
const allowCloud = ref(false)
const allowWeb = ref(false)
const sending = ref(false)
const setup = ref(false)
const remote = computed(() => !['none', 'ollama'].includes(effective.value.kind) && !preset.value)
const provider = computed(() => providerLabels[effective.value.kind] ?? effective.value.kind)
const activePreset = computed(() => presets.find(p => p.value === preset.value))
const placeholder = computed(() => activePreset.value?.hint ?? 'Sort a folder, explain a document, or find something out…')
const canSend = computed(() => !sending.value && !needsConnection.value && !!prompt.value.trim() && (!remote.value || allowCloud.value))

watch(assignedPet, async petId => { if (petId !== null) { try { const folder = await taskCall<string | null>('pet_folder', { petId }); if (folder) roots.value = [folder] } catch { /* shared error */ } } }, { immediate: true })
watch(droppedFiles, paths => { attachments.value = [...new Set([...attachments.value, ...paths])] })

function shortcut(kind: string) {
  if (kind === 'organise') { preset.value = 'sort_type'; prompt.value = 'Organise my folder' }
  else { preset.value = ''; prompt.value = kind === 'summarise' ? 'Summarise the attached document and extract the key points.' : 'Research '; allowWeb.value = kind === 'research' }
}
function pickPreset(event: Event) {
  const value = (event.target as HTMLSelectElement).value
  ;(event.target as HTMLSelectElement).value = ''
  const chosen = presets.find(p => p.value === value)
  if (!chosen) return
  preset.value = chosen.value
  if (chosen.value.startsWith('sort')) prompt.value = chosen.hint
}
function clearPreset() { preset.value = ''; prompt.value = '' }
async function pick(folders: boolean) {
  try { const selected = await taskCall<string[]>('task_pick', { folders }); if (folders) roots.value = [...new Set([...roots.value, ...selected])]; else attachments.value = [...new Set([...attachments.value, ...selected])] } catch { /* shared error */ }
}
async function submit() {
  if (!canSend.value) return
  sending.value = true
  try {
    const request: TaskRequest = { prompt: prompt.value, roots: roots.value, attachments: attachments.value, preset: preset.value, mode: mode.value, parentId: parent.value, petId: assignedPet.value ?? pets.value[0]?.id ?? null, allowCloud: allowCloud.value, allowWeb: allowWeb.value }
    const task = await taskCall<Task>('task_start', { request })
    tasks.value = [task, ...tasks.value.filter(t => t.id !== task.id)]; prompt.value = ''; parent.value = null
  } catch { /* shared error */ } finally { sending.value = false }
}
function followup(task: Task) { parent.value = task.id; preset.value = ''; roots.value = [...task.request.roots]; attachments.value = [...task.request.attachments]; prompt.value = ''; document.getElementById('task-prompt')?.focus() }
watch(pendingFollowup, task => { if (task) { followup(task); pendingFollowup.value = null } }, { immediate: true })
function short(path: string) { return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path }

</script>

<template>
  <div class="assistant-page">
  <section class="page-heading">
    <div><h1>Assistant</h1><p>Pick who helps, then ask for what you need.</p></div>
  </section>
  <div v-if="taskError" class="error-banner" role="alert">{{ taskError }}<button aria-label="Dismiss task error" @click="taskError = ''"><X :size="15" /></button></div>
  <PetPicker @setup="setup = true" />
  <div v-if="setup" class="setup-drawer">
    <div class="setup-drawer-head"><h2>Keys & apps</h2><button type="button" class="text-link" @click="setup = false">Done</button></div>
    <ConnectionsPanel />
  </div>

  <form class="task-composer" @submit.prevent="submit">
    <p v-if="parent" class="followup-note">Following up on an earlier request <button type="button" class="text-link" @click="parent = null">Start fresh</button></p>
    <div v-if="activePreset" class="preset-pill">
      <Sparkles :size="13" /> {{ activePreset.label }} <span>· runs on this computer, no AI</span>
      <button type="button" aria-label="Stop using this action" @click="clearPreset"><X :size="13" /></button>
    </div>
    <label for="task-prompt" class="sr-only">Your request</label>
    <textarea id="task-prompt" v-model="prompt" rows="3" maxlength="20000" required :placeholder="placeholder" @keydown.meta.enter="submit" @keydown.ctrl.enter="submit" />
    <ul v-if="roots.length || attachments.length" class="attachment-list">
      <li v-for="path in roots" :key="path" :title="path"><FolderPlus :size="12" /> {{ short(path) }} <button type="button" :aria-label="`Remove ${path}`" @click="roots = roots.filter(p => p !== path)"><X :size="11" /></button></li>
      <li v-for="path in attachments" :key="path" :title="path"><Paperclip :size="12" /> {{ short(path) }} <button type="button" :aria-label="`Remove ${path}`" @click="attachments = attachments.filter(p => p !== path)"><X :size="11" /></button></li>
    </ul>
    <p v-if="mode === 'advanced'" class="task-problem">This AI can run commands on your computer. You'll approve each step, but some can't be undone.</p>
    <label v-if="remote" class="check-label consent"><input v-model="allowCloud" type="checkbox"> Let {{ provider }} read this request and anything attached</label>
    <div class="composer-toolbar">
      <button type="button" class="tool-button" title="Add a folder" aria-label="Add folder" @click="pick(true)"><FolderPlus :size="16" /></button>
      <button type="button" class="tool-button" title="Attach files" aria-label="Attach files" @click="pick(false)"><Paperclip :size="16" /></button>
      <button v-if="!preset" type="button" class="tool-button web-toggle" :aria-pressed="allowWeb" title="Let it search the web" @click="allowWeb = !allowWeb"><Globe :size="16" /> Web</button>
      <button class="primary-button send-button" :disabled="!canSend" :aria-label="sending ? 'Sending' : 'Send request'"><ArrowUp :size="17" /></button>
    </div>
  </form>

  <div class="task-shortcuts" aria-label="Suggestions">
    <button type="button" @click="shortcut('organise')"><FolderPlus :size="14" /> Organise files</button>
    <button type="button" @click="shortcut('summarise')"><Sparkles :size="14" /> Summarise</button>
    <button type="button" @click="shortcut('research')"><Search :size="14" /> Research</button>
    <select class="shortcut-select" aria-label="More actions" value="" @change="pickPreset">
      <option value="" disabled>More actions…</option>
      <option v-for="option in presets" :key="option.value" :value="option.value">{{ option.label }}</option>
    </select>
  </div>

  <p v-if="needsConnection" class="connection-hint">No AI picked yet. Try <strong>Organise files</strong> or another built-in action, or choose an AI above to ask anything.</p>
  </div>
</template>

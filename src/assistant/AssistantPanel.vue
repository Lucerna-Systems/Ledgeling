<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { ArrowUp, FolderPlus, Globe, Paperclip, Pencil, Plus, Search, Sparkles, X } from '@lucide/vue'
import { pets } from '../state'
import { activeConversation, conversations, renameConversation, tasks, taskError, assignedPet, droppedFiles, effectiveConnection, livePet, providerReady, taskCall } from './state'
import { presets, providerLabels, type Task, type TaskRequest } from './types'
import ChatTurn from './ChatTurn.vue'
import PetFace from './PetFace.vue'
import PetPicker from './PetPicker.vue'
import './assistant.css'

const emit = defineEmits<{ providers: [] }>()

const prompt = ref('')
const roots = ref<string[]>([])
const attachments = ref<string[]>([])
const preset = ref('')
// Every gate follows the connection the chosen companion would actually
// run on — a per-pet override changes the mode and consent rules too.
const effective = computed(() => effectiveConnection(assignedPet.value))
const mode = computed(() => !preset.value && ['codex', 'claude'].includes(effective.value.kind) ? 'advanced' : 'standard')
const needsConnection = computed(() => !preset.value && effective.value.kind === 'none')
// Consent to send content to a provider is given once, when it is added.
const termsDue = computed(() => !preset.value && !providerReady(effective.value.kind))
const allowWeb = ref(false)
const sending = ref(false)
const provider = computed(() => providerLabels[effective.value.kind] ?? effective.value.kind)
const activePreset = computed(() => presets.find(p => p.value === preset.value))
const placeholder = computed(() => activePreset.value?.hint ?? (chat.value ? `Reply to ${chat.value.pet?.name.split(' ')[0] ?? 'your pet'}…` : 'Sort a folder, explain a document, or find something out…'))
const canSend = computed(() => !sending.value && !needsConnection.value && !termsDue.value && !!prompt.value.trim())

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
    const request: TaskRequest = { prompt: prompt.value, roots: roots.value, attachments: attachments.value, preset: preset.value, mode: mode.value, parentId: chat.value?.turns.at(-1)?.id ?? null, petId: assignedPet.value ?? pets.value[0]?.id ?? null, allowCloud: false, allowWeb: allowWeb.value }
    const task = await taskCall<Task>('task_start', { request })
    tasks.value = [task, ...tasks.value.filter(t => t.id !== task.id)]; prompt.value = ''; attachments.value = []; preset.value = ''
    if (!chat.value) activeConversation.value = task.id
  } catch { /* shared error */ } finally { sending.value = false }
}

const chat = computed(() => conversations.value.find(c => c.id === activeConversation.value) ?? null)
const chatPetName = computed(() => chat.value?.pet?.name ?? pets.value.find(p => p.id === chat.value?.petId)?.name ?? 'any pet')
const renaming = ref(false)
const titleDraft = ref('')
const titleField = ref<HTMLInputElement | null>(null)
watch(activeConversation, () => {
  const current = chat.value
  if (!current) return
  const last = current.turns.at(-1)!
  assignedPet.value = livePet(current.petId, current.pet)
  roots.value = [...last.request.roots]
  attachments.value = []
  preset.value = ''
  renaming.value = false
}, { immediate: true })

const thread = ref<HTMLElement | null>(null)
watch(() => [activeConversation.value, chat.value?.turns.length, chat.value?.turns.at(-1)?.status], async () => {
  await nextTick()
  if (thread.value) thread.value.scrollTop = thread.value.scrollHeight
}, { immediate: true })

async function startRename() {
  if (!chat.value) return
  titleDraft.value = chat.value.title
  renaming.value = true
  await nextTick()
  titleField.value?.select()
}
async function commitRename() {
  if (!renaming.value || !chat.value) return
  renaming.value = false
  const title = titleDraft.value.trim()
  if (title !== chat.value.title) { try { await renameConversation(chat.value.id, title) } catch { /* shared error */ } }
}
/** Chats send on Enter like any messenger; Shift+Enter still breaks the line. */
function sendOnEnter(event: KeyboardEvent) {
  if (!chat.value || event.isComposing) return
  event.preventDefault()
  void submit()
}
function short(path: string) { return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path }

</script>

<template>
  <div class="assistant-page" :class="{ chatting: chat }">
  <section v-if="chat" class="chat-header">
    <PetFace :pet="chat.pet" :pet-id="chat.petId" :size="40" />
    <div class="chat-heading">
      <input v-if="renaming" ref="titleField" v-model="titleDraft" maxlength="80" aria-label="Conversation name"
             @keydown.enter.prevent="commitRename" @keydown.esc.prevent="renaming = false" @blur="commitRename">
      <h1 v-else @dblclick="startRename">{{ chat.title }}</h1>
      <p>With {{ chatPetName }} · {{ chat.turns.length }} {{ chat.turns.length === 1 ? 'request' : 'requests' }}</p>
    </div>
    <button v-if="!renaming" type="button" class="icon-button" aria-label="Rename conversation" @click="startRename"><Pencil :size="15" /></button>
    <button type="button" class="new-chat-button" @click="activeConversation = null"><Plus :size="15" /> New chat</button>
  </section>
  <section v-else class="page-heading">
    <div><h1>Assistant</h1><p>Pick who helps, then ask for what you need.</p></div>
  </section>
  <div v-if="taskError" class="error-banner" role="alert">{{ taskError }}<button aria-label="Dismiss task error" @click="taskError = ''"><X :size="15" /></button></div>
  <div v-if="chat" ref="thread" class="chat-thread" aria-live="polite">
    <ChatTurn v-for="task in chat.turns" :key="task.id" :task="task" :pet="chat.pet" />
  </div>
  <PetPicker v-else @providers="emit('providers')" />

  <form class="task-composer" @submit.prevent="submit">
    <div v-if="activePreset" class="preset-pill">
      <Sparkles :size="13" /> {{ activePreset.label }} <span>· runs on this computer, no AI</span>
      <button type="button" aria-label="Stop using this action" @click="clearPreset"><X :size="13" /></button>
    </div>
    <label for="task-prompt" class="sr-only">Your request</label>
    <textarea id="task-prompt" v-model="prompt" rows="3" maxlength="20000" required :placeholder="placeholder" @keydown.meta.enter="submit" @keydown.ctrl.enter="submit" @keydown.enter.exact="sendOnEnter" />
    <ul v-if="roots.length || attachments.length" class="attachment-list">
      <li v-for="path in roots" :key="path" :title="path"><FolderPlus :size="12" /> {{ short(path) }} <button type="button" :aria-label="`Remove ${path}`" @click="roots = roots.filter(p => p !== path)"><X :size="11" /></button></li>
      <li v-for="path in attachments" :key="path" :title="path"><Paperclip :size="12" /> {{ short(path) }} <button type="button" :aria-label="`Remove ${path}`" @click="attachments = attachments.filter(p => p !== path)"><X :size="11" /></button></li>
    </ul>
    <p v-if="mode === 'advanced'" class="task-problem">This AI can run commands on your computer. You'll approve each step, but some can't be undone.</p>
    <p v-if="termsDue" class="task-problem">{{ provider }} needs its terms accepted once before it can run. <button type="button" class="text-link" @click="emit('providers')">Open AI providers</button></p>
    <div class="composer-toolbar">
      <button type="button" class="tool-button" title="Add a folder" aria-label="Add folder" @click="pick(true)"><FolderPlus :size="16" /></button>
      <button type="button" class="tool-button" title="Attach files" aria-label="Attach files" @click="pick(false)"><Paperclip :size="16" /></button>
      <button v-if="!preset" type="button" class="tool-button web-toggle" :aria-pressed="allowWeb" title="Let it search the web" @click="allowWeb = !allowWeb"><Globe :size="16" /> Web</button>
      <button class="primary-button send-button" :disabled="!canSend" :aria-label="sending ? 'Sending' : 'Send request'"><ArrowUp :size="17" /></button>
    </div>
  </form>

  <div v-if="!chat" class="task-shortcuts" aria-label="Suggestions">
    <button type="button" @click="shortcut('organise')"><FolderPlus :size="14" /> Organise files</button>
    <button type="button" @click="shortcut('summarise')"><Sparkles :size="14" /> Summarise</button>
    <button type="button" @click="shortcut('research')"><Search :size="14" /> Research</button>
    <select class="shortcut-select" aria-label="More actions" value="" @change="pickPreset">
      <option value="" disabled>More actions…</option>
      <option v-for="option in presets" :key="option.value" :value="option.value">{{ option.label }}</option>
    </select>
  </div>

  <p v-if="needsConnection" class="connection-hint">No AI picked yet. Try <strong>Organise files</strong> or another built-in action, or <button type="button" class="text-link" @click="emit('providers')">set up an AI provider</button> to ask anything.</p>
  </div>
</template>

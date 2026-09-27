<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { Bot, MessageSquarePlus, Send } from '@lucide/vue'
import ChatTurn from '../../assistant/ChatTurn.vue'
import '../../assistant/assistant.css'
import { conversations, effectiveConnection, startTasks, taskCall, taskError } from '../../assistant/state'
import { providerLabels, type Task, type TaskRequest } from '../../assistant/types'
import { chatDraft } from '../chatDraft'
import { current, currentPet, root } from '../state'

const props = defineProps<{ active?: boolean }>()
const prompt = ref('')
const allowCloud = ref(false)
const sending = ref(false)
/** The chat on screen, by its first task; null starts a new one. */
const chatId = ref<number | null>(null)
const scroller = ref<HTMLElement | null>(null)

const petId = computed(() => currentPet.value?.id ?? null)
const connection = computed(() => effectiveConnection(petId.value))
const mode = computed(() => ['codex', 'claude'].includes(connection.value.kind) ? 'advanced' : 'standard')
const remote = computed(() => !['none', 'ollama'].includes(connection.value.kind))
const provider = computed(() => providerLabels[connection.value.kind] ?? connection.value.kind)

/** Only chats that worked in this worktree. */
const here = computed(() => conversations.value.filter(c => c.turns[0]?.request.roots.includes(root.value ?? '')))
const chat = computed(() => here.value.find(c => c.id === chatId.value) ?? null)
watch(root, () => { chatId.value = here.value[0]?.id ?? null })
watch(here, list => { if (chatId.value === null && list[0] && !prompt.value) chatId.value = list[0].id }, { once: true })
watch(chatDraft, text => { if (text) { prompt.value = text; chatId.value = null; chatDraft.value = null } }, { immediate: true })
watch(() => chat.value?.turns.length, () => void nextTick(() => scroller.value?.scrollTo({ top: scroller.value.scrollHeight, behavior: 'smooth' })))

async function send() {
  const text = prompt.value.trim()
  if (!text || !root.value || sending.value) return
  sending.value = true
  const request: TaskRequest = {
    prompt: text, roots: [root.value], attachments: [], preset: '', mode: mode.value,
    parentId: chat.value?.turns.at(-1)?.id ?? null, petId: petId.value, allowCloud: allowCloud.value, allowWeb: false,
  }
  try {
    const task = await taskCall<Task>('task_start', { request })
    if (!chat.value) chatId.value = task.id
    prompt.value = ''
  } catch { /* shown below */ } finally { sending.value = false }
}

let stop = () => {}
onMounted(async () => { stop = await startTasks() })
onUnmounted(() => stop())
</script>

<template>
  <section class="code-chat">
    <div class="panel-bar">
      <h2 class="grow">{{ chat?.title ?? 'New chat' }}</h2>
      <select v-if="here.length" :value="chatId ?? ''" aria-label="Chats in this worktree" style="max-width:48%" @change="chatId = Number(($event.target as HTMLSelectElement).value) || null">
        <option value="">New chat</option>
        <option v-for="c in here" :key="c.id" :value="c.id">{{ c.title }}</option>
      </select>
      <button class="icon" title="New chat" @click="chatId = null"><MessageSquarePlus :size="14" /></button>
    </div>
    <div ref="scroller" class="panel-scroll panel-pad">
      <template v-if="chat">
        <ChatTurn v-for="turn in chat.turns" :key="turn.id" :task="turn" :pet="chat.pet" />
      </template>
      <div v-else class="empty">
        <span class="glyph"><Bot :size="22" /></span>
        <h2>Ask about {{ current?.worktree.name }}</h2>
        <p v-if="connection.kind === 'none'">Connect an AI in Ledgeling's Assistant settings first. Or use Claude Code in the Terminal tab.</p>
        <p v-else>{{ provider }} works in this worktree's folder{{ currentPet ? `, and ${currentPet.name} shows when it is busy` : '' }}. You approve every change before it happens.</p>
      </div>
    </div>
    <p v-if="taskError" class="banner error">{{ taskError }}</p>
    <form class="composer" @submit.prevent="send">
      <textarea v-model="prompt" :disabled="connection.kind === 'none'" placeholder="Explain this code, fix a bug, add a test…" aria-label="Message" @keydown.meta.enter="send" @keydown.ctrl.enter="send" />
      <label v-if="remote" class="consent"><input v-model="allowCloud" type="checkbox"> Let {{ provider }} read this request and the worktree's files</label>
      <div class="row-actions">
        <span class="muted small" style="flex:1">{{ provider }}{{ mode === 'advanced' ? ' · can run commands, with your approval' : '' }}</span>
        <button class="primary-button" type="submit" :disabled="sending || !prompt.trim() || connection.kind === 'none' || (remote && !allowCloud)"><Send :size="13" /> Send</button>
      </div>
    </form>
  </section>
</template>

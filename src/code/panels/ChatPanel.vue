<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { MessageSquarePlus, Radar, Send } from '@lucide/vue'
import ChatTurn from '../../assistant/ChatTurn.vue'
import PetFace from '../../assistant/PetFace.vue'
import '../../assistant/assistant.css'
import { conversations, effectiveConnection, startTasks, taskCall, taskError } from '../../assistant/state'
import { providerLabels, type Task, type TaskRequest } from '../../assistant/types'
import { chatDraft } from '../chatDraft'
import { current, currentPet, root } from '../state'
import { markSeen, notes } from '../petNotes'
import { digest, digestText, type Digest } from '../digest'

/** The worktree's pet as a side chat: what it noticed happening there, what
 *  is going on right now, and questions for the chat model. */
const props = defineProps<{ active?: boolean }>()
const prompt = ref('')
const allowCloud = ref(false)
const share = ref(true)
const sending = ref(false)
/** The chat on screen, by its first task; null starts a new one. */
const chatId = ref<number | null>(null)
const scroller = ref<HTMLElement | null>(null)

const petId = computed(() => currentPet.value?.id ?? null)
const petName = computed(() => currentPet.value?.name ?? 'Ledgeling')
const petFace = computed(() => currentPet.value ? { packId: currentPet.value.packId, name: currentPet.value.name } : null)
const connection = computed(() => effectiveConnection(petId.value))
const mode = computed(() => ['codex', 'claude'].includes(connection.value.kind) ? 'advanced' : 'standard')
const remote = computed(() => !['none', 'ollama'].includes(connection.value.kind))
const provider = computed(() => providerLabels[connection.value.kind] ?? connection.value.kind)

/** Only chats that worked in this worktree. */
const here = computed(() => conversations.value.filter(c => c.turns[0]?.request.roots.includes(root.value ?? '')))
const chat = computed(() => here.value.find(c => c.id === chatId.value) ?? null)
watch(root, () => { chatId.value = here.value[0]?.id ?? null; checks.value = [] })
watch(here, list => { if (chatId.value === null && list[0] && !prompt.value) chatId.value = list[0].id }, { once: true })
watch(chatDraft, text => { if (text) { prompt.value = text; chatId.value = null; chatDraft.value = null } }, { immediate: true })

/** "What's going on?" answers, kept for this visit to the worktree. */
const checks = ref<{ at: number; digest: Digest }[]>([])
function check() {
  if (!root.value) return
  checks.value = [...checks.value, { at: Date.now(), digest: digest(root.value) }]
}

type Item =
  | { kind: 'note'; at: number; key: string; text: string; tone: string }
  | { kind: 'check'; at: number; key: string; digest: Digest }
  | { kind: 'turn'; at: number; key: string; task: Task }
/** Notes, checks and chat turns in the order they happened. */
const feed = computed<Item[]>(() => [
  ...notes.value.map(n => ({ kind: 'note' as const, at: n.at, key: `n${n.id}`, text: n.text, tone: n.tone })),
  ...checks.value.map(c => ({ kind: 'check' as const, at: c.at, key: `c${c.at}`, digest: c.digest })),
  ...(chat.value?.turns ?? []).map(t => ({ kind: 'turn' as const, at: t.createdAt * 1000, key: `t${t.id}`, task: t })),
].sort((a, b) => a.at - b.at))

watch(() => feed.value.length, () => void nextTick(() => scroller.value?.scrollTo({ top: scroller.value.scrollHeight, behavior: 'smooth' })))
watch([() => props.active, notes], ([active]) => { if (active) markSeen() }, { immediate: true })

const time = (at: number) => new Date(at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })

async function send() {
  const text = prompt.value.trim()
  if (!text || !root.value || sending.value) return
  sending.value = true
  const request: TaskRequest = {
    prompt: text, roots: [root.value], attachments: [], preset: '', mode: mode.value,
    parentId: chat.value?.turns.at(-1)?.id ?? null, petId: petId.value, allowCloud: allowCloud.value, allowWeb: false,
    context: share.value ? `What is happening in the worktree ${current.value?.worktree.name} right now:\n${digestText(digest(root.value), notes.value)}` : '',
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
  <section class="code-chat pet-chat">
    <div class="panel-bar pet-head">
      <PetFace :pet="petFace" :pet-id="petId" :size="28" />
      <div class="grow">
        <h2>{{ petName }}</h2>
        <small class="muted">{{ currentPet ? 'Looking after' : 'Working in' }} {{ current?.worktree.name }}</small>
      </div>
      <select v-if="here.length" :value="chatId ?? ''" aria-label="Chats in this worktree" style="max-width:40%" @change="chatId = Number(($event.target as HTMLSelectElement).value) || null">
        <option value="">New chat</option>
        <option v-for="c in here" :key="c.id" :value="c.id">{{ c.title }}</option>
      </select>
      <button class="icon" title="New chat" @click="chatId = null"><MessageSquarePlus :size="14" /></button>
    </div>

    <div ref="scroller" class="panel-scroll panel-pad">
      <div v-if="!feed.length" class="empty">
        <PetFace :pet="petFace" :pet-id="petId" :size="44" />
        <h2>{{ petName }} is keeping an eye on {{ current?.worktree.name }}</h2>
        <p>{{ currentPet ? 'It notes' : 'Notes appear here' }} when Claude Code needs you, when a build finishes or fails, and what is being served. Ask what's going on at any time.</p>
        <button class="primary-button" @click="check"><Radar :size="14" /> What's going on?</button>
      </div>
      <template v-for="item in feed" :key="item.key">
        <div v-if="item.kind === 'note'" class="pet-note" :class="item.tone">
          <PetFace :pet="petFace" :pet-id="petId" :size="20" />
          <p>{{ item.text }}</p>
          <time>{{ time(item.at) }}</time>
        </div>
        <div v-else-if="item.kind === 'check'" class="pet-check">
          <div class="pet-check-head"><PetFace :pet="petFace" :pet-id="petId" :size="20" /><strong>Here's what's going on</strong><time>{{ time(item.at) }}</time></div>
          <p v-if="item.digest.branch" class="muted small">On {{ item.digest.branch }}</p>
          <h3>Terminals</h3>
          <p v-if="!item.digest.terminals.length" class="muted">Nothing is running.</p>
          <div v-for="t in item.digest.terminals" :key="t.title" class="pet-term">
            <div><strong>{{ t.title }}</strong> <span class="chip" :class="{ ask: t.state === 'waiting for you', busy: t.state === 'working', live: t.state === 'running' }">{{ t.state }}</span></div>
            <pre v-if="t.tail.length">{{ t.tail.join('\n') }}</pre>
          </div>
          <template v-if="item.digest.serving.length">
            <h3>Serving</h3>
            <p v-for="s in item.digest.serving" :key="s">{{ s }}</p>
          </template>
          <h3>Changes</h3>
          <p v-if="!item.digest.changes.length" class="muted">Nothing uncommitted.</p>
          <p v-for="c in item.digest.changes" :key="c" class="mono">{{ c }}</p>
        </div>
        <ChatTurn v-else :task="item.task" :pet="chat?.pet ?? null" />
      </template>
    </div>

    <p v-if="taskError" class="banner error">{{ taskError }}</p>
    <form class="composer" @submit.prevent="send">
      <textarea v-model="prompt" :disabled="connection.kind === 'none'" :placeholder="connection.kind === 'none' ? 'Connect an AI in the Assistant settings to ask questions' : `Ask ${petName} about this worktree…`" aria-label="Message" @keydown.meta.enter="send" @keydown.ctrl.enter="send" />
      <label class="consent"><input v-model="share" type="checkbox"> Include what's going on here: terminals' last lines, ports and changes</label>
      <label v-if="remote" class="consent"><input v-model="allowCloud" type="checkbox"> Let {{ provider }} read this request{{ share ? ', that summary' : '' }} and the worktree's files</label>
      <div class="row-actions">
        <button type="button" class="ghost" title="A summary made on this computer, without AI" @click="check"><Radar :size="13" /> What's going on?</button>
        <span class="muted small" style="flex:1">{{ connection.kind === 'none' ? '' : provider }}</span>
        <button class="primary-button" type="submit" :disabled="sending || !prompt.trim() || connection.kind === 'none' || (remote && !allowCloud)"><Send :size="13" /> Send</button>
      </div>
    </form>
  </section>
</template>

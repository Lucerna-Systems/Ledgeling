<script setup lang="ts">
import { computed, ref } from 'vue'
import MarkdownResult from './MarkdownResult.vue'
import { taskCall, refreshTasks } from './state'
import { providerLabels, terminal, type Task } from './types'
const props = defineProps<{ task: Task }>()
const emit = defineEmits<{ followup: [task: Task] }>()
const expanded = ref(false)
const longResult = computed(() => props.task.result.length > 600)
const preview = computed(() => props.task.result
  .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
  .replace(/[#*_`>]/g, '').replace(/\s+/g, ' ').trim().slice(0, 190) + '…')
const pending = ref(false)
const canUndo = computed(() => terminal(props.task.status) && props.task.operations.some(o => ['applied', 'applying', 'undoing'].includes(o.state)))
async function control(command: string, args = {}) {
  pending.value = true
  try { await taskCall(command, { id: props.task.id, ...args }); await refreshTasks() } catch { /* shared error banner */ } finally { pending.value = false }
}
const statusLabels: Record<string, string> = {
  queued: 'Starting', running: 'Working', waiting: 'Needs you', cancelling: 'Stopping', completed: 'Done',
  failed: "Didn't work", cancelled: 'Cancelled', interrupted: 'Interrupted', undone: 'Undone',
}
const meta = computed(() => {
  const who = props.task.request.preset ? 'Built-in action' : providerLabels[props.task.connection.kind] ?? props.task.connection.kind
  const created = new Date(props.task.createdAt * 1000)
  const sameDay = created.toDateString() === new Date().toDateString()
  const when = sameDay ? created.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : created.toLocaleString(undefined, { day: 'numeric', month: 'short', hour: 'numeric', minute: '2-digit' })
  return [who, props.task.request.mode === 'advanced' ? 'Agent' : '', when].filter(Boolean).join(' · ')
})
async function open(url: string) { try { await taskCall('task_open_url', { url }) } catch { /* shared error banner */ } }
</script>
<template>
  <article class="task-card" :aria-label="`Task ${task.id}`">
    <header><strong>{{ task.request.prompt }}</strong><span class="task-status" :data-status="task.status">{{ statusLabels[task.status] ?? task.status }}</span></header>
    <p class="task-meta">{{ meta }}</p>
    <p v-if="task.progress" role="status">{{ task.progress }}</p>
    <div v-if="task.error" class="task-problem" role="alert">{{ task.error }}</div>
    <section v-if="task.approval && task.status === 'waiting'" class="approval-card" aria-label="Pending approval">
      <h3>{{ task.approval.summary.headline || task.approval.title }}</h3>
      <ul class="approval-items" :class="{ mono: task.approval.summary.monospace }">
        <li v-for="(item, index) in task.approval.summary.items" :key="index">{{ item }}</li>
        <li v-if="task.approval.summary.more" class="approval-more">+{{ task.approval.summary.more }} more</li>
      </ul>
      <p v-if="task.approval.summary.note">{{ task.approval.summary.note }}</p>
      <details v-if="task.approval.details.operations?.length">
        <summary>Every change · {{ task.approval.details.operations.length }}</summary>
        <ol class="operation-list"><li v-for="(op, index) in task.approval.details.operations" :key="index">
          <span>{{ op.kind === 'move' ? 'Move' : 'Create' }}</span>
          <code v-if="op.source">{{ op.source }} → </code><code>{{ op.destination }}</code>
          <details v-if="op.content"><summary>Preview contents</summary><pre>{{ op.content }}</pre></details>
        </li></ol>
      </details>
      <div class="task-actions">
        <button class="primary-button" :disabled="pending" @click="control('task_approve', { approvalId: task.approval.id, accept: true })">{{ task.approval.summary.accept || 'Approve' }}</button>
        <button :disabled="pending" @click="control('task_approve', { approvalId: task.approval.id, accept: false })">Reject</button>
      </div>
    </section>
    <div v-if="task.result" class="task-result">
      <p v-if="longResult && !expanded" class="task-result-preview">{{ preview }}</p>
      <MarkdownResult v-else :text="task.result" @open="open" />
      <button v-if="longResult" class="text-link" :aria-expanded="expanded" @click="expanded = !expanded">{{ expanded ? 'Collapse response' : 'Read full response' }}</button>
    </div>
    <ul v-if="task.sources.length" class="task-sources"><li v-for="source in task.sources" :key="source.url"><button class="text-link" @click="open(source.url)">{{ source.title }} ↗</button><small>{{ source.url }}</small></li></ul>
    <details v-if="task.operations.length"><summary>Operation journal · {{ task.operations.length }} changes</summary><ul><li v-for="(op, index) in task.operations" :key="index">{{ op.state }} · {{ op.destination }}</li></ul></details>
    <div class="task-actions">
      <button v-if="!terminal(task.status)" :disabled="pending || task.status === 'cancelling'" @click="control('task_cancel')">Cancel task</button>
      <button v-if="canUndo" :disabled="pending" @click="control('task_undo')">Undo file changes</button>
      <button v-if="terminal(task.status)" @click="emit('followup', task)">Follow up</button>
    </div>
  </article>
</template>

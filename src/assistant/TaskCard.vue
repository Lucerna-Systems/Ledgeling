<script setup lang="ts">
import { computed, ref } from 'vue'
import MarkdownResult from './MarkdownResult.vue'
import { taskCall, refreshTasks } from './state'
import { terminal, type Task } from './types'
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
async function open(url: string) { try { await taskCall('task_open_url', { url }) } catch { /* shared error banner */ } }
</script>
<template>
  <article class="task-card" :aria-label="`Task ${task.id}`">
    <header><strong>{{ task.request.prompt }}</strong><span class="task-status" :data-status="task.status">{{ task.status }}</span></header>
    <p class="task-meta">{{ task.request.preset ? 'Built-in · local' : task.connection.kind }} · {{ task.request.mode }} · {{ new Date(task.createdAt * 1000).toLocaleString() }}</p>
    <p role="status">{{ task.progress }}</p>
    <div v-if="task.error" class="task-problem" role="alert">{{ task.error }}</div>
    <section v-if="task.approval && task.status === 'waiting'" class="approval-card" aria-label="Pending approval">
      <h3>{{ task.approval.title }}</h3>
      <template v-if="task.approval.details.operations">
        <p>Only these changes will be applied. Existing files will not be overwritten.</p>
        <ol class="operation-list"><li v-for="(op, index) in task.approval.details.operations" :key="index">
          <span>{{ op.kind === 'move' ? 'Move' : 'Create' }}</span>
          <code v-if="op.source">{{ op.source }} → </code><code>{{ op.destination }}</code>
          <details v-if="op.content"><summary>Preview contents</summary><pre>{{ op.content }}</pre></details>
        </li></ol>
      </template>
      <pre v-else>{{ JSON.stringify(task.approval.details, null, 2) }}</pre>
      <div class="task-actions">
        <button class="primary-button" :disabled="pending" @click="control('task_approve', { approvalId: task.approval.id, accept: true })">Approve</button>
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

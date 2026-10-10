<script setup lang="ts">
import { computed } from 'vue'
import type { AgentStatus } from './types'

/** What an agent says it is doing, as a small pill in its pane header. */
const props = defineProps<{ status: AgentStatus }>()
const view = computed(() => {
  const { state, detail } = props.status
  if (state === 'working') return { tone: 'busy', label: detail ? `Using ${detail}` : 'Working' }
  if (state === 'waiting') return { tone: 'ask', label: 'Needs you' }
  if (state === 'done') return { tone: 'live', label: 'Done' }
  return null
})
const title = computed(() => props.status.state === 'waiting' && props.status.detail ? props.status.detail : undefined)
</script>

<template>
  <span v-if="view" class="chip" :class="view.tone" :title="title" role="status"><span class="dot" :class="view.tone" />{{ view.label }}</span>
</template>

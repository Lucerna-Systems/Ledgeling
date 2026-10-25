<script setup lang="ts">
import { computed } from 'vue'
import { CircleCheck, CircleDot, CircleSlash, GitMerge, GitPullRequest, GitPullRequestClosed, GitPullRequestDraft } from '@lucide/vue'

const props = defineProps<{ kind: 'pr' | 'issue' | 'draft' | 'redacted'; state: string | null | undefined; draft?: boolean; reason?: string | null; size?: number }>()
const look = computed(() => {
  const state = (props.state ?? '').toUpperCase()
  if (props.kind === 'pr') {
    if (state === 'MERGED') return { icon: GitMerge, tone: 'merged', label: 'Merged' }
    if (state === 'CLOSED') return { icon: GitPullRequestClosed, tone: 'closed', label: 'Closed' }
    if (props.draft) return { icon: GitPullRequestDraft, tone: 'muted', label: 'Draft' }
    return { icon: GitPullRequest, tone: 'open', label: 'Open' }
  }
  if (props.kind === 'issue') {
    if (state === 'CLOSED') return props.reason === 'NOT_PLANNED'
      ? { icon: CircleSlash, tone: 'muted', label: 'Closed as not planned' }
      : { icon: CircleCheck, tone: 'merged', label: 'Closed as completed' }
    return { icon: CircleDot, tone: 'open', label: 'Open' }
  }
  return { icon: CircleDot, tone: 'muted', label: props.kind === 'draft' ? 'Draft card' : 'Hidden' }
})
</script>

<template>
  <component :is="look.icon" class="gh-state-icon" :class="look.tone" :size="size ?? 15" :aria-label="look.label" />
</template>

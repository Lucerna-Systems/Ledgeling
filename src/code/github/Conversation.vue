<script setup lang="ts">
import { computed, ref } from 'vue'
import { Check, CircleAlert, Eye, LoaderCircle, Send } from '@lucide/vue'
import { mac } from '../shortcuts'
import Avatar from './Avatar.vue'
import GhMarkdown from './GhMarkdown.vue'
import { ago } from './format'
import { act, canWrite, follow, gh, repo } from './store'
import type { Actor, Comment, Kind, Review } from './types'

const props = defineProps<{
  kind: Kind; number: number; author: Actor | null; body: string; createdAt: string
  comments: Comment[]; reviews?: Review[]; open: boolean
}>()
const emit = defineEmits<{ changed: [] }>()

interface Entry { key: string; who: string | null; body: string; at: string; review?: Review['state'] }
const entries = computed<Entry[]>(() => {
  const all: Entry[] = props.comments.map((c, i) => ({ key: `c${i}`, who: c.author?.login ?? null, body: c.body, at: c.createdAt }))
  for (const [i, r] of (props.reviews ?? []).entries()) {
    if (r.state === 'PENDING') continue
    all.push({ key: `r${i}`, who: r.author?.login ?? null, body: r.body, at: r.submittedAt, review: r.state })
  }
  return all.sort((a, b) => a.at.localeCompare(b.at))
})
const reviewWords: Record<string, string> = {
  APPROVED: 'approved these changes', CHANGES_REQUESTED: 'requested changes', COMMENTED: 'reviewed', DISMISSED: 'had a review dismissed',
}

const text = ref('')
const preview = ref(false)
const busy = ref<string | null>(null)
const mine = computed(() => props.author?.login && props.author.login === gh.value?.account)

async function send(what: 'comment' | 'approve' | 'request_changes') {
  if (busy.value) return
  busy.value = what
  try {
    if (what === 'comment') await act({ action: 'comment', kind: props.kind, number: props.number, body: text.value })
    else await act({ action: 'review', number: props.number, event: what, body: text.value })
    text.value = ''
    preview.value = false
    emit('changed')
  } catch { /* shown above */ } finally { busy.value = null }
}
</script>

<template>
  <div class="gh-thread">
    <article class="gh-entry opener">
      <Avatar :login="author?.login" :size="28" class="gh-entry-avatar" />
      <div class="gh-bubble">
        <header><strong>{{ author?.login ?? 'ghost' }}</strong><span class="muted">opened {{ ago(createdAt) }}</span></header>
        <GhMarkdown :text="body" :repo-url="repo?.url" @link="follow" />
      </div>
    </article>

    <template v-for="entry in entries" :key="entry.key">
      <div v-if="entry.review && !entry.body.trim()" class="gh-event" :class="entry.review.toLowerCase()">
        <span class="gh-event-dot"><Check v-if="entry.review === 'APPROVED'" :size="11" /><CircleAlert v-else-if="entry.review === 'CHANGES_REQUESTED'" :size="11" /><Eye v-else :size="11" /></span>
        <Avatar :login="entry.who" :size="16" /><strong>{{ entry.who ?? 'ghost' }}</strong>
        <span class="muted">{{ reviewWords[entry.review] }} · {{ ago(entry.at) }}</span>
      </div>
      <article v-else class="gh-entry" :class="entry.review?.toLowerCase()">
        <Avatar :login="entry.who" :size="28" class="gh-entry-avatar" />
        <div class="gh-bubble">
          <header>
            <strong>{{ entry.who ?? 'ghost' }}</strong>
            <span v-if="entry.review" class="gh-review-tag" :class="entry.review.toLowerCase()">{{ reviewWords[entry.review] }}</span>
            <span class="muted">{{ ago(entry.at) }}</span>
          </header>
          <GhMarkdown :text="entry.body" :repo-url="repo?.url" @link="follow" />
        </div>
      </article>
    </template>

    <div class="gh-composer">
      <div class="gh-composer-tabs">
        <span class="segmented-mini" role="group" aria-label="Composer view">
          <button :aria-pressed="!preview" @click="preview = false">Write</button>
          <button :aria-pressed="preview" :disabled="!text.trim()" @click="preview = true">Preview</button>
        </span>
        <span class="muted small">Markdown works here</span>
      </div>
      <GhMarkdown v-if="preview" class="gh-preview" :text="text" :repo-url="repo?.url" @link="follow" />
      <textarea
        v-else v-model="text" class="gh-textarea" :placeholder="kind === 'pr' ? 'Leave a comment or review' : 'Leave a comment'"
        aria-label="Comment" @keydown.meta.enter.prevent="send('comment')" @keydown.ctrl.enter.prevent="send('comment')"
      />
      <div class="gh-composer-actions">
        <template v-if="kind === 'pr' && open && !mine && canWrite">
          <button :disabled="!!busy || !text.trim()" title="Request changes: say what should change" @click="send('request_changes')"><CircleAlert :size="13" /> Request changes</button>
          <button class="gh-approve" :disabled="!!busy" @click="send('approve')"><Check :size="13" /> Approve</button>
        </template>
        <span class="grow" />
        <button class="primary-button" :disabled="!!busy || !text.trim()" :title="`Comment (${mac ? '⌘' : 'Ctrl+'}↵)`" @click="send('comment')">
          <Send v-if="busy !== 'comment'" :size="13" /><LoaderCircle v-else :size="13" class="spin" /> Comment
        </button>
      </div>
    </div>
  </div>
</template>

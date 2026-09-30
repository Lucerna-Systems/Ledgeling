<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { CircleCheck, CircleX, GitBranch, Inbox, LoaderCircle, MessageSquare, Search, X } from '@lucide/vue'
import Avatar from './Avatar.vue'
import StateIcon from './StateIcon.vue'
import { ago, labelStyle, rollup } from './format'
import { issues, open, prs, repo, show } from './store'
import type { Filter, IssueSummary, Kind, PrSummary } from './types'

const props = defineProps<{ kind: Kind }>()
const list = computed(() => props.kind === 'pr' ? prs : issues)
const items = computed(() => list.value.items.value as (PrSummary | IssueSummary)[])
const filters = computed<{ id: Filter; label: string }[]>(() => [
  { id: 'open', label: 'Open' },
  { id: 'closed', label: 'Closed' },
  ...(props.kind === 'pr' ? [{ id: 'merged' as Filter, label: 'Merged' }] : []),
  { id: 'all', label: 'All' },
])
/** GitHub's own search words, offered as one-click filters. */
const quick = computed(() => props.kind === 'pr'
  ? [{ label: 'Mine', query: 'author:@me' }, { label: 'Review requested', query: 'review-requested:@me' }, { label: 'Assigned', query: 'assignee:@me' }]
  : [{ label: 'Mine', query: 'author:@me' }, { label: 'Assigned', query: 'assignee:@me' }, { label: 'Unassigned', query: 'no:assignee' }])

const typed = ref(list.value.search.value)
let timer: ReturnType<typeof setTimeout> | undefined
watch(typed, text => {
  clearTimeout(timer)
  timer = setTimeout(() => { list.value.search.value = text }, 350)
})
watch(() => [list.value.filter.value, list.value.search.value], () => void list.value.load())
function toggleQuick(query: string) {
  const words = typed.value.split(/\s+/).filter(Boolean)
  typed.value = (words.includes(query) ? words.filter(w => w !== query) : [...words, query]).join(' ')
}
const quickOn = (query: string) => typed.value.split(/\s+/).includes(query)

onMounted(() => { if (!items.value.length) void list.value.load() })

const isPr = (item: PrSummary | IssueSummary): item is PrSummary => 'headRefName' in item
const selected = (item: PrSummary | IssueSummary) => open.value?.kind === props.kind && open.value.number === item.number
function pick(item: PrSummary | IssueSummary) { show(props.kind, item.number) }
function keys(event: KeyboardEvent) {
  if (!['ArrowDown', 'ArrowUp', 'j', 'k'].includes(event.key) || (event.target as HTMLElement).tagName === 'INPUT') return
  event.preventDefault()
  const at = items.value.findIndex(selected)
  const next = items.value[Math.min(items.value.length - 1, Math.max(0, at + (event.key === 'ArrowDown' || event.key === 'j' ? 1 : -1)))]
  if (next) { pick(next); document.getElementById(`gh-row-${next.number}`)?.scrollIntoView({ block: 'nearest' }) }
}
</script>

<template>
  <div class="gh-list" tabindex="-1" @keydown="keys">
    <div class="gh-filters">
      <label class="gh-search">
        <Search :size="13" class="muted" />
        <input v-model="typed" :placeholder="kind === 'pr' ? 'Search pull requests' : 'Search issues'" :aria-label="kind === 'pr' ? 'Search pull requests' : 'Search issues'" spellcheck="false">
        <button v-if="typed" class="icon" title="Clear search" @click="typed = ''"><X :size="12" /></button>
      </label>
      <div class="gh-filter-row">
        <span class="segmented-mini" role="group" aria-label="State">
          <button v-for="f in filters" :key="f.id" :aria-pressed="list.filter.value === f.id" @click="list.filter.value = f.id">{{ f.label }}</button>
        </span>
        <span class="gh-quick">
          <button v-for="q in quick" :key="q.query" class="gh-pill" :aria-pressed="quickOn(q.query)" :title="q.query" @click="toggleQuick(q.query)">{{ q.label }}</button>
        </span>
      </div>
    </div>

    <div class="gh-rows" :class="{ dim: list.loading.value && items.length }">
      <div v-if="list.loading.value && !items.length" class="gh-skeletons" aria-label="Loading">
        <div v-for="n in 6" :key="n" class="gh-skeleton"><span /><span /></div>
      </div>
      <div v-else-if="!items.length" class="empty">
        <span class="glyph"><Inbox :size="22" /></span>
        <h2>{{ typed ? 'Nothing matches' : kind === 'pr' ? `No ${list.filter.value === 'all' ? '' : list.filter.value + ' '}pull requests` : `No ${list.filter.value === 'all' ? '' : list.filter.value + ' '}issues` }}</h2>
        <p v-if="typed">Try fewer words, or clear the search.</p>
        <p v-else>in {{ repo?.nameWithOwner }}</p>
      </div>
      <button
        v-for="item in items" :id="`gh-row-${item.number}`" :key="item.number"
        class="bare gh-row" :class="{ selected: selected(item) }" :aria-current="selected(item)" @click="pick(item)"
      >
        <StateIcon :kind="kind" :state="item.state" :draft="isPr(item) && item.isDraft" :reason="!isPr(item) ? item.stateReason : null" class="gh-row-icon" />
        <span class="gh-row-main">
          <span class="gh-row-title">
            <strong>{{ item.title }}</strong>
            <span v-for="label in item.labels.slice(0, 3)" :key="label.name" class="gh-label" :style="labelStyle(label.color)">{{ label.name }}</span>
          </span>
          <small class="gh-row-meta">
            <span class="mono">#{{ item.number }}</span>
            <template v-if="isPr(item)"><span class="gh-branch"><GitBranch :size="10" />{{ item.headRefName }}</span></template>
            <span class="gh-by"><Avatar :login="item.author?.login" :size="14" />{{ item.author?.login ?? 'ghost' }}</span>
            <span>{{ ago(item.updatedAt) }}</span>
          </small>
        </span>
        <span class="gh-row-side">
          <template v-if="isPr(item)">
            <LoaderCircle v-if="rollup(item.statusCheckRollup).state === 'pending'" :size="13" class="gh-check pending" aria-label="Checks running" />
            <CircleX v-else-if="rollup(item.statusCheckRollup).state === 'fail'" :size="13" class="gh-check fail" aria-label="Checks failing" />
            <CircleCheck v-else-if="rollup(item.statusCheckRollup).state === 'pass'" :size="13" class="gh-check pass" aria-label="Checks passed" />
            <span v-if="item.reviewDecision === 'APPROVED'" class="gh-mini-chip good">Approved</span>
            <span v-else-if="item.reviewDecision === 'CHANGES_REQUESTED'" class="gh-mini-chip bad">Changes</span>
          </template>
          <span v-if="item.comments.length" class="gh-comments"><MessageSquare :size="11" />{{ item.comments.length }}</span>
          <span v-if="item.assignees.length" class="gh-stack"><Avatar v-for="a in item.assignees.slice(0, 2)" :key="a.login" :login="a.login" :size="16" /></span>
        </span>
      </button>
      <p v-if="items.length >= 100" class="muted small gh-more">Showing the 100 most recent. Search to narrow it down.</p>
    </div>
  </div>
</template>

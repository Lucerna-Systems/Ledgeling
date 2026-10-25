<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { ArrowLeft, Bot, CircleCheck, CircleSlash, ExternalLink, GitBranchPlus, GitPullRequest, LoaderCircle, Pencil, RefreshCw, RotateCcw } from '@lucide/vue'
import { code, current, refreshState, select, tab, worktrees } from '../state'
import { pendingTerminal } from '../terminalFocus'
import { has } from '../tools'
import Avatar from './Avatar.vue'
import Conversation from './Conversation.vue'
import Menu from './Menu.vue'
import MetaBar from './MetaBar.vue'
import StateIcon from './StateIcon.vue'
import { ago, issueBranch, shellQuote } from './format'
import { act, canWrite, carry, follow, gh, open, openOnGitHub, repo, show, view } from './store'
import type { Issue, MenuItem } from './types'

const props = defineProps<{ number: number; back?: boolean }>()
const issue = ref<Issue | null>(null)
const failed = ref('')
const busy = ref('')

async function load(quiet = false) {
  if (!quiet) { issue.value = null; failed.value = '' }
  try {
    const next = await view<Issue>('issue', props.number)
    // A link to /issues/N may name a pull request; GitHub redirects, so do we.
    if (/\/pull\/\d+/.test(next.url)) { show('pr', props.number); return }
    if (next.number === props.number) issue.value = next
  } catch (e) { failed.value = String(e) }
}
watch(() => props.number, () => void load(), { immediate: true })

const mine = computed(() => issue.value?.author?.login === gh.value?.account)
const closedAs = computed(() => issue.value?.stateReason === 'NOT_PLANNED' ? 'not planned' : 'completed')
const branch = computed(() => issue.value ? issueBranch(issue.value.number, issue.value.title) : '')
const existing = computed(() => worktrees.value.find(e => e.worktree.branch === branch.value) ?? null)

async function run(label: string, work: () => Promise<unknown>) {
  if (busy.value) return
  busy.value = label
  try { await work(); await load(true) } catch { /* shown above */ } finally { busy.value = '' }
}

/** A worktree of its own on `issue-N-title`, from the default branch, with a free pet. */
async function startWork(withClaude: boolean) {
  const project = current.value?.project
  if (!issue.value || !project) return
  let path = existing.value?.worktree.path ?? null
  if (!path) {
    busy.value = 'start'
    try {
      const base = `origin/${repo.value?.defaultBranch ?? 'main'}`
      // origin/<default> may not be fetched yet; the project's HEAD is the fallback.
      path = await invoke<string>('code_worktree_create', { project: project.path, branch: branch.value, base, petId: null })
        .catch(() => code<string>('code_worktree_create', { project: project.path, branch: branch.value, base: '', petId: null }))
    } catch { busy.value = ''; return }
    busy.value = ''
  }
  carry.value = { kind: 'issue', number: props.number }
  await refreshState()
  await select(path)
  if (withClaude && issue.value) {
    const n = issue.value.number
    const prompt = `Work on GitHub issue #${n}: "${issue.value.title}". Read it first with \`gh issue view ${n} --comments\`, then make the change on this branch and summarise what you did.`
    pendingTerminal.value = { worktree: path, kind: 'agent', title: 'Claude Code', command: `claude ${shellQuote(prompt)}` }
    tab.value = 'terminal'
  }
}

const closeMenu = computed<MenuItem[]>(() => [
  { label: 'Close as completed', hint: 'Done, fixed or resolved', icon: CircleCheck, run: () => run('close', () => act({ action: 'close', kind: 'issue', number: props.number, reason: 'completed' })) },
  { label: 'Close as not planned', hint: 'Won’t fix, duplicate or stale', icon: CircleSlash, run: () => run('close', () => act({ action: 'close', kind: 'issue', number: props.number, reason: 'not_planned' })) },
])

const editing = ref(false)
const title = ref('')
function editTitle() { title.value = issue.value?.title ?? ''; editing.value = true }
async function saveTitle() {
  // Enter saves and the input's blur follows; only the first counts.
  if (!editing.value) return
  editing.value = false
  if (!issue.value || !title.value.trim() || title.value === issue.value.title) return
  await run('title', () => act({ action: 'edit', kind: 'issue', number: props.number, title: title.value, body: null }))
}
const closeDetail = () => { open.value = null }
</script>

<template>
  <div class="gh-detail">
    <div v-if="failed" class="banner error"><p>{{ failed }}</p><button class="ghost" @click="load()">Try again</button></div>
    <div v-else-if="!issue" class="gh-loading"><LoaderCircle :size="16" class="spin" /> Loading #{{ number }}…</div>
    <template v-else>
      <header class="gh-detail-head">
        <button v-if="back" class="bare gh-back" @click="closeDetail"><ArrowLeft :size="13" /> All issues</button>
        <div class="gh-title-row">
          <input v-if="editing" v-model="title" v-focus class="gh-title-input" aria-label="Title" @keydown.enter="saveTitle" @keydown.esc="editing = false" @blur="saveTitle">
          <h1 v-else class="gh-title">{{ issue.title }} <span class="muted">#{{ issue.number }}</span></h1>
          <button v-if="!editing && (mine || canWrite)" class="icon" title="Edit title" @click="editTitle"><Pencil :size="13" /></button>
        </div>
        <div class="gh-subline">
          <span class="gh-state-badge" :class="issue.state === 'OPEN' ? 'open' : issue.stateReason === 'NOT_PLANNED' ? 'closed-muted' : 'merged'">
            <StateIcon kind="issue" :state="issue.state" :reason="issue.stateReason" :size="13" />{{ issue.state === 'OPEN' ? 'Open' : `Closed as ${closedAs}` }}
          </span>
          <span class="gh-by"><Avatar :login="issue.author?.login" :size="16" /><strong>{{ issue.author?.login }}</strong></span>
          <span class="muted">opened {{ ago(issue.createdAt) }} · {{ issue.comments.length }} comment{{ issue.comments.length === 1 ? '' : 's' }}</span>
        </div>
        <div class="gh-actions">
          <template v-if="issue.state === 'OPEN'">
            <button class="primary-button" :disabled="!!busy" :title="existing ? `Switch to ${existing.worktree.name}` : `New worktree on ${branch}, with a free pet`" @click="startWork(false)">
              <LoaderCircle v-if="busy === 'start'" :size="13" class="spin" /><GitBranchPlus v-else :size="13" /> {{ existing ? 'Go to its worktree' : 'Start work' }}
            </button>
            <Menu v-if="has('claude')" align="left" title="Start work with Claude Code" :items="[
              { label: existing ? 'Hand it to Claude Code' : 'Start work with Claude Code', hint: `Claude Code reads #${issue.number} and works on ${branch}`, icon: Bot, run: () => startWork(true) },
            ]"><Bot :size="13" /></Menu>
          </template>
          <span class="grow" />
          <button class="icon" title="Reload" :disabled="!!busy" @click="load(true)"><RefreshCw :size="13" /></button>
          <button class="icon" title="Open on GitHub" @click="openOnGitHub(issue.url)"><ExternalLink :size="13" /></button>
          <template v-if="mine || canWrite">
            <Menu v-if="issue.state === 'OPEN'" label="Close" :items="closeMenu" />
            <button v-else class="ghost" :disabled="!!busy" @click="run('reopen', () => act({ action: 'reopen', kind: 'issue', number: props.number }))"><RotateCcw :size="13" /> Reopen</button>
          </template>
        </div>
      </header>

      <div class="gh-detail-body">
        <MetaBar kind="issue" :number="issue.number" :labels="issue.labels" :assignees="issue.assignees" :milestone="issue.milestone?.title" @changed="load(true)" />
        <div v-if="issue.closedByPullRequestsReferences.length" class="gh-linked">
          <GitPullRequest :size="12" class="muted" /><span class="muted">Fixed by</span>
          <button v-for="p in issue.closedByPullRequestsReferences" :key="p.number" class="gh-pill" @click="follow(p.url)">#{{ p.number }}</button>
        </div>
        <Conversation kind="issue" :number="issue.number" :author="issue.author" :body="issue.body" :created-at="issue.createdAt" :comments="issue.comments" :open="issue.state === 'OPEN'" @changed="load(true)" />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowLeft, Bot, Check, CircleCheck, CircleDashed, CircleX, Copy, ExternalLink, FolderGit2, GitBranch, GitMerge, GitPullRequestClosed, GitPullRequestDraft, LoaderCircle, Pencil, RefreshCw, RotateCcw, SkipForward } from '@lucide/vue'
import { ask as confirm } from '../../terminal/confirm'
import { refreshState, select, status as gitStatus, tab, worktrees } from '../state'
import { pendingTerminal } from '../terminalFocus'
import { has } from '../tools'
import Avatar from './Avatar.vue'
import Conversation from './Conversation.vue'
import DiffView from './DiffView.vue'
import Menu from './Menu.vue'
import MetaBar from './MetaBar.vue'
import StateIcon from './StateIcon.vue'
import { ago, checkName, checkState, checkUrl, prState, rollup, rollupText, runId, shellQuote } from './format'
import { act, ask, canWrite, carry, follow, gh, open, openOnGitHub, repo, view } from './store'
import type { MenuItem, Pr } from './types'

const props = defineProps<{ number: number; back?: boolean }>()
const pr = ref<Pr | null>(null)
const failed = ref('')
const pane = ref<'conversation' | 'files' | 'checks' | 'commits'>('conversation')
const busy = ref('')

async function load(quiet = false) {
  if (!quiet) { pr.value = null; failed.value = '' }
  try {
    const next = await view<Pr>('pr', props.number)
    if (next.number === props.number) pr.value = next
  } catch (e) { failed.value = String(e) }
}
watch(() => props.number, () => { pane.value = 'conversation'; void load() }, { immediate: true })

const state = computed(() => pr.value ? prState(pr.value) : 'open')
const checks = computed(() => rollup(pr.value?.statusCheckRollup))
const reviewers = computed(() => (pr.value?.reviewRequests ?? []).map(r => r.login ?? r.name ?? r.slug ?? '').filter(Boolean))
/** The worktree that has this pull request's branch checked out, if any. */
const localTree = computed(() => pr.value ? worktrees.value.find(e => e.worktree.branch === pr.value!.headRefName) ?? null : null)
const onScreen = computed(() => !!pr.value && gitStatus.value?.branch === pr.value.headRefName)
const mine = computed(() => pr.value?.author?.login === gh.value?.account)

async function run(label: string, work: () => Promise<unknown>) {
  if (busy.value) return
  busy.value = label
  try { await work(); await load(true) } catch { /* shown above */ } finally { busy.value = '' }
}

async function checkout() {
  if (!pr.value) return
  if (localTree.value) { carry.value = { kind: 'pr', number: props.number }; await select(localTree.value.worktree.path); return }
  await run('checkout', async () => {
    const path = await ask<string>('code_gh_checkout', { number: props.number })
    carry.value = { kind: 'pr', number: props.number }
    await refreshState()
    await select(path)
  })
}

// ------------------------------------------------------------------ merging
type Method = 'squash' | 'merge' | 'rebase'
const method = ref<Method>((() => { try { return (localStorage.getItem('ledgeling.code.mergeMethod') as Method) || 'squash' } catch { return 'squash' } })())
watch(method, m => { try { localStorage.setItem('ledgeling.code.mergeMethod', m) } catch { /* not kept */ } })
const deleteBranch = ref(true)
const methodWords: Record<Method, string> = { squash: 'Squash and merge', merge: 'Create a merge commit', rebase: 'Rebase and merge' }
const blocked = computed(() => pr.value?.mergeable === 'CONFLICTING' || checks.value.state === 'fail' || pr.value?.reviewDecision === 'CHANGES_REQUESTED' || pr.value?.reviewDecision === 'REVIEW_REQUIRED')
const mergeReady = computed(() => !!pr.value && pr.value.mergeable === 'MERGEABLE' && ['CLEAN', 'HAS_HOOKS', 'UNSTABLE'].includes(pr.value.mergeStateStatus))

async function merge(auto: boolean) {
  if (!pr.value) return
  const what = auto
    ? `Merge #${pr.value.number} automatically once its requirements pass?`
    : `${methodWords[method.value]} #${pr.value.number} into ${pr.value.baseRefName}?${deleteBranch.value ? ` The ${pr.value.headRefName} branch will be deleted on GitHub.` : ''}`
  if (!await confirm(what, auto ? 'Enable auto-merge' : methodWords[method.value])) return
  await run('merge', () => act({ action: 'merge', number: props.number, method: method.value, deleteBranch: deleteBranch.value, auto }))
}

const more = computed<MenuItem[]>(() => {
  if (!pr.value) return []
  const items: MenuItem[] = []
  if (state.value === 'open' && (mine.value || canWrite.value)) items.push({ label: 'Convert to draft', icon: GitPullRequestDraft, run: () => run('draft', () => act({ action: 'ready', number: props.number, ready: false })) })
  if (state.value === 'open' || state.value === 'draft') items.push({ label: 'Close pull request', icon: GitPullRequestClosed, danger: true, run: async () => { if (await confirm(`Close #${props.number} without merging?`, 'Close')) await run('close', () => act({ action: 'close', kind: 'pr', number: props.number })) } })
  if (state.value === 'closed') items.push({ label: 'Reopen pull request', icon: RotateCcw, run: () => run('reopen', () => act({ action: 'reopen', kind: 'pr', number: props.number })) })
  items.push({ label: 'Copy branch name', icon: Copy, hint: pr.value.headRefName, run: () => navigator.clipboard?.writeText(pr.value!.headRefName) })
  return items
})

/** Hands the review to the user's own Claude Code, in the pull request's worktree. */
function claude(what: 'review' | 'address') {
  if (!pr.value || !localTree.value) return
  const n = pr.value.number
  const prompt = what === 'review'
    ? `Review pull request #${n} (${pr.value.title}). Read it with \`gh pr view ${n}\` and \`gh pr diff ${n}\`, then tell me what you would change and why. Do not edit files yet.`
    : `Address the review feedback on pull request #${n}. Read it with \`gh pr view ${n} --comments\`, make the changes on this branch, and summarise what you did.`
  pendingTerminal.value = { worktree: localTree.value.worktree.path, kind: 'agent', title: 'Claude Code', command: `claude ${shellQuote(prompt)}` }
  tab.value = 'terminal'
}

// ------------------------------------------------------------------- title
const editing = ref(false)
const title = ref('')
function editTitle() { title.value = pr.value?.title ?? ''; editing.value = true }
async function saveTitle() {
  // Enter saves and the input's blur follows; only the first counts.
  if (!editing.value) return
  editing.value = false
  if (!pr.value || !title.value.trim() || title.value === pr.value.title) return
  await run('title', () => act({ action: 'edit', kind: 'pr', number: props.number, title: title.value, body: null }))
}

const closeDetail = () => { open.value = null }
const rerunFailed = () => run('rerun', () => Promise.all(failedRuns.value.map(id => act({ action: 'rerunFailed', run: id }))))
const failedRuns = computed(() => [...new Set((pr.value?.statusCheckRollup ?? []).filter(c => checkState(c) === 'fail').map(c => runId(checkUrl(c))).filter((id): id is number => id !== null))])
const duration = (start?: string, end?: string) => {
  if (!start || !end) return ''
  const s = Math.max(0, Math.round((new Date(end).getTime() - new Date(start).getTime()) / 1000))
  return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${s % 60}s`
}
</script>

<template>
  <div class="gh-detail">
    <div v-if="failed" class="banner error"><p>{{ failed }}</p><button class="ghost" @click="load()">Try again</button></div>
    <div v-else-if="!pr" class="gh-loading"><LoaderCircle :size="16" class="spin" /> Loading #{{ number }}…</div>
    <template v-else>
      <header class="gh-detail-head">
        <button v-if="back" class="bare gh-back" @click="closeDetail"><ArrowLeft :size="13" /> All pull requests</button>
        <div class="gh-title-row">
          <input v-if="editing" v-model="title" v-focus class="gh-title-input" aria-label="Title" @keydown.enter="saveTitle" @keydown.esc="editing = false" @blur="saveTitle">
          <h1 v-else class="gh-title">{{ pr.title }} <span class="muted">#{{ pr.number }}</span></h1>
          <button v-if="!editing && (mine || canWrite)" class="icon" title="Edit title" @click="editTitle"><Pencil :size="13" /></button>
        </div>
        <div class="gh-subline">
          <span class="gh-state-badge" :class="state"><StateIcon kind="pr" :state="pr.state" :draft="pr.isDraft" :size="13" />{{ { open: 'Open', draft: 'Draft', merged: 'Merged', closed: 'Closed' }[state] }}</span>
          <span class="gh-by"><Avatar :login="pr.author?.login" :size="16" /><strong>{{ pr.author?.login }}</strong></span>
          <span class="muted">{{ state === 'merged' ? 'merged' : 'wants to merge' }} {{ pr.commits.length }} commit{{ pr.commits.length === 1 ? '' : 's' }} into</span>
          <span class="gh-ref">{{ pr.baseRefName }}</span><span class="muted">from</span><span class="gh-ref" :title="pr.isCrossRepository ? `${pr.headRepositoryOwner?.login}:${pr.headRefName}` : pr.headRefName">{{ pr.isCrossRepository ? `${pr.headRepositoryOwner?.login}:` : '' }}{{ pr.headRefName }}</span>
        </div>
        <div class="gh-actions">
          <button v-if="onScreen" class="ghost gh-here" disabled><FolderGit2 :size="13" /> This worktree</button>
          <button v-else-if="state !== 'merged' && state !== 'closed'" :disabled="!!busy" :title="localTree ? `Switch to ${localTree.worktree.name}` : 'Check out into a new worktree beside the project, with a free pet'" @click="checkout">
            <LoaderCircle v-if="busy === 'checkout'" :size="13" class="spin" /><FolderGit2 v-else :size="13" />
            {{ localTree ? 'Switch to its worktree' : 'Check out in a worktree' }}
          </button>
          <Menu v-if="localTree && has('claude')" label="Claude Code" :items="[
            { label: 'Review this pull request', hint: 'Reads the diff and reports, without editing', icon: Bot, run: () => claude('review') },
            { label: 'Address review comments', hint: 'Makes the requested changes on this branch', icon: Bot, run: () => claude('address') },
          ]"><Bot :size="13" /></Menu>
          <span class="grow" />
          <button class="icon" title="Reload" :disabled="!!busy" @click="load(true)"><RefreshCw :size="13" /></button>
          <button class="icon" title="Open on GitHub" @click="openOnGitHub(pr.url)"><ExternalLink :size="13" /></button>
          <Menu :items="more" title="More actions" />
        </div>
      </header>

      <section v-if="state === 'open' || state === 'draft'" class="gh-merge" :class="{ ok: mergeReady && !blocked, bad: blocked }">
        <div class="gh-merge-lines">
          <p>
            <CircleX v-if="checks.state === 'fail'" :size="14" class="gh-check fail" />
            <LoaderCircle v-else-if="checks.state === 'pending'" :size="14" class="gh-check pending" />
            <CircleCheck v-else-if="checks.state === 'pass'" :size="14" class="gh-check pass" />
            <CircleDashed v-else :size="14" class="muted" />
            <button class="bare gh-link" @click="pane = 'checks'">{{ rollupText(checks) }}</button>
          </p>
          <p>
            <CircleCheck v-if="pr.reviewDecision === 'APPROVED'" :size="14" class="gh-check pass" />
            <CircleX v-else-if="pr.reviewDecision === 'CHANGES_REQUESTED'" :size="14" class="gh-check fail" />
            <CircleDashed v-else :size="14" class="muted" />
            {{ pr.reviewDecision === 'APPROVED' ? 'Approved' : pr.reviewDecision === 'CHANGES_REQUESTED' ? 'Changes requested' : pr.reviewDecision === 'REVIEW_REQUIRED' ? 'Review required' : 'No review needed' }}
          </p>
          <p>
            <CircleX v-if="pr.mergeable === 'CONFLICTING'" :size="14" class="gh-check fail" />
            <CircleCheck v-else-if="pr.mergeable === 'MERGEABLE'" :size="14" class="gh-check pass" />
            <LoaderCircle v-else :size="14" class="gh-check pending" />
            {{ pr.mergeable === 'CONFLICTING' ? 'Conflicts with ' + pr.baseRefName : pr.mergeable === 'MERGEABLE' ? 'No conflicts' : 'GitHub is checking for conflicts' }}
          </p>
        </div>
        <div v-if="state === 'draft'" class="gh-merge-actions">
          <span class="muted small grow">This is a draft, so it cannot be merged yet.</span>
          <button v-if="mine || canWrite" class="primary-button" :disabled="!!busy" @click="run('ready', () => act({ action: 'ready', number: pr!.number, ready: true }))"><Check :size="13" /> Ready for review</button>
        </div>
        <div v-else-if="canWrite" class="gh-merge-actions">
          <select v-model="method" aria-label="Merge method" class="gh-method">
            <option value="squash">Squash and merge</option>
            <option value="merge">Merge commit</option>
            <option value="rebase">Rebase and merge</option>
          </select>
          <label class="gh-check-label"><input v-model="deleteBranch" type="checkbox"> Delete branch</label>
          <span class="grow" />
          <button v-if="pr.autoMergeRequest" class="ghost" disabled>Auto-merge is on</button>
          <button v-else-if="!mergeReady" :disabled="!!busy" title="Merge by itself once checks and reviews pass" @click="merge(true)"><SkipForward :size="13" /> Merge when ready</button>
          <button class="primary-button gh-merge-button" :disabled="!!busy || pr.mergeable === 'CONFLICTING'" @click="merge(false)">
            <LoaderCircle v-if="busy === 'merge'" :size="13" class="spin" /><GitMerge v-else :size="13" /> {{ methodWords[method] }}
          </button>
        </div>
      </section>
      <section v-else-if="state === 'merged'" class="gh-merge merged">
        <p><GitMerge :size="14" /> <strong>{{ pr.mergedBy?.login ?? 'Someone' }}</strong> merged this {{ ago(pr.mergedAt) }}.</p>
      </section>

      <nav class="gh-detail-tabs" role="tablist">
        <button role="tab" :aria-selected="pane === 'conversation'" @click="pane = 'conversation'">Conversation <span class="gh-count">{{ pr.comments.length + pr.reviews.filter(r => r.state !== 'PENDING').length }}</span></button>
        <button role="tab" :aria-selected="pane === 'files'" @click="pane = 'files'">Files <span class="gh-count">{{ pr.changedFiles }}</span><span class="counts"><span class="plus">+{{ pr.additions }}</span><span class="minus">−{{ pr.deletions }}</span></span></button>
        <button role="tab" :aria-selected="pane === 'checks'" @click="pane = 'checks'">Checks <span class="gh-count" :class="checks.state">{{ checks.total }}</span></button>
        <button role="tab" :aria-selected="pane === 'commits'" @click="pane = 'commits'">Commits <span class="gh-count">{{ pr.commits.length }}</span></button>
      </nav>

      <div class="gh-detail-body">
        <template v-if="pane === 'conversation'">
          <MetaBar kind="pr" :number="pr.number" :labels="pr.labels" :assignees="pr.assignees" :reviewers="reviewers" :milestone="pr.milestone?.title" @changed="load(true)" />
          <div v-if="pr.closingIssuesReferences.length" class="gh-linked">
            <span class="muted">Closes</span>
            <button v-for="i in pr.closingIssuesReferences" :key="i.number" class="gh-pill" @click="follow(i.url)">#{{ i.number }}</button>
          </div>
          <Conversation kind="pr" :number="pr.number" :author="pr.author" :body="pr.body" :created-at="pr.createdAt" :comments="pr.comments" :reviews="pr.reviews" :open="state === 'open'" @changed="load(true)" />
        </template>
        <DiffView v-else-if="pane === 'files'" :number="pr.number" :local="onScreen" />
        <div v-else-if="pane === 'checks'" class="gh-checks">
          <div v-if="failedRuns.length && canWrite" class="gh-checks-bar">
            <span class="muted small grow">{{ rollupText(checks) }}</span>
            <button :disabled="!!busy" @click="rerunFailed"><RotateCcw :size="13" /> Re-run failed jobs</button>
          </div>
          <p v-if="!pr.statusCheckRollup?.length" class="muted gh-pad">No checks ran on this pull request.</p>
          <button v-for="(c, i) in pr.statusCheckRollup ?? []" :key="i" class="bare gh-check-row" :disabled="!checkUrl(c)" @click="checkUrl(c) && openOnGitHub(checkUrl(c)!)">
            <CircleCheck v-if="checkState(c) === 'pass'" :size="15" class="gh-check pass" />
            <CircleX v-else-if="checkState(c) === 'fail'" :size="15" class="gh-check fail" />
            <LoaderCircle v-else-if="checkState(c) === 'pending'" :size="15" class="gh-check pending" />
            <SkipForward v-else :size="15" class="muted" />
            <span class="grow"><strong>{{ checkName(c) }}</strong><small v-if="c.workflowName" class="muted">{{ c.workflowName }}</small></span>
            <span class="muted small mono">{{ duration(c.startedAt, c.completedAt) }}</span>
            <ExternalLink v-if="checkUrl(c)" :size="12" class="muted" />
          </button>
        </div>
        <div v-else class="gh-commits">
          <div v-for="c in pr.commits" :key="c.oid" class="gh-commit">
            <Avatar :login="c.authors[0]?.login ?? c.authors[0]?.name" :size="22" />
            <span class="grow"><strong>{{ c.messageHeadline }}</strong><small class="muted">{{ c.authors.map(a => a.login || a.name).join(', ') }} · {{ ago(c.authoredDate) }}</small></span>
            <button class="bare mono gh-sha" title="Open the commit on GitHub" @click="openOnGitHub(`${repo?.url}/commit/${c.oid}`)"><GitBranch :size="10" />{{ c.oid.slice(0, 7) }}</button>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

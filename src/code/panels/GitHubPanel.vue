<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { CircleCheck, CircleDot, CircleX, Download, ExternalLink, GitBranch, GitPullRequest, GitPullRequestArrow, LoaderCircle, LogIn, Plus, RefreshCw, SquareKanban, TriangleAlert, X } from '@lucide/vue'
import { root, status as gitStatus, tab } from '../state'
import { pendingTerminal } from '../terminalFocus'
import { loadTools, tools } from '../tools'
import Avatar from '../github/Avatar.vue'
import BoardView from '../github/BoardView.vue'
import IssueDetail from '../github/IssueDetail.vue'
import ItemList from '../github/ItemList.vue'
import Menu from '../github/Menu.vue'
import NewIssue from '../github/NewIssue.vue'
import NewPr from '../github/NewPr.vue'
import PrDetail from '../github/PrDetail.vue'
import { prState, rollup, rollupText } from '../github/format'
import { branchPr, checking, composing, gh, ghError, issues, open, openOnGitHub, prs, ready, refreshGh, refreshLists, repo, section, show, startGithub } from '../github/store'
import type { Section } from '../github/types'
import '../github/github.css'

const props = defineProps<{ active?: boolean }>()
onMounted(() => { startGithub(); void loadTools() })

/** Wide enough for the list and the open item side by side. */
const host = ref<HTMLElement | null>(null)
const width = ref(0)
const observer = new ResizeObserver(([entry]) => { width.value = entry.contentRect.width })
watch(host, (el, old) => { if (old) observer.unobserve(old); if (el) observer.observe(el) })
onBeforeUnmount(() => observer.disconnect())
const split = computed(() => width.value >= 780)
const showingDetail = computed(() => !!open.value || !!composing.value)

// Signing in happens in the browser; coming back to the window checks again.
function recheck() { if (props.active && gh.value && !ready.value) void refreshGh().then(refreshLists) }
onMounted(() => window.addEventListener('focus', recheck))
onBeforeUnmount(() => window.removeEventListener('focus', recheck))
watch(() => props.active, now => { if (now && ready.value) refreshLists() })

const ghTool = computed(() => tools.value.find(t => t.id === 'gh'))
function install() {
  pendingTerminal.value = { tool: 'gh', worktree: root.value ?? '', title: 'Install gh' }
  tab.value = 'terminal'
}
function signIn() {
  pendingTerminal.value = { worktree: root.value ?? '', kind: 'shell', title: 'GitHub sign-in', command: 'gh auth login --hostname github.com --web --git-protocol https --scopes project' }
  tab.value = 'terminal'
}

const sections = computed<{ id: Section; label: string; icon: unknown; count?: number }[]>(() => [
  { id: 'prs', label: 'Pull requests', icon: GitPullRequest, count: prs.filter.value === 'open' && !prs.search.value ? prs.items.value.length : undefined },
  { id: 'issues', label: 'Issues', icon: CircleDot, count: issues.filter.value === 'open' && !issues.search.value ? issues.items.value.length : undefined },
  { id: 'boards', label: 'Boards', icon: SquareKanban },
])
function pickSection(id: Section) {
  section.value = id
  if (open.value && (id === 'boards' || (id === 'prs') !== (open.value.kind === 'pr'))) open.value = null
  composing.value = null
}

/** The branch on screen, when it is not the default one, may deserve a pull request. */
const branch = computed(() => gitStatus.value?.branch ?? null)
const featureBranch = computed(() => !!branch.value && !!repo.value && branch.value !== repo.value.defaultBranch)
const branchChecks = computed(() => rollup(branchPr.value?.statusCheckRollup))
const newItems = computed(() => [
  { label: 'New issue', icon: CircleDot, run: () => { open.value = null; composing.value = 'issue'; if (section.value === 'boards') section.value = 'issues' } },
  { label: 'New pull request', hint: featureBranch.value ? `From ${branch.value}` : 'Switch to a worktree on a branch first', icon: GitPullRequestArrow, disabled: !featureBranch.value || !!branchPr.value, run: () => { startPr(); section.value = 'prs' } },
])
const dismissError = () => { ghError.value = '' }
function startPr() { open.value = null; composing.value = 'pr' }
</script>

<template>
  <section ref="host" class="gh-panel">
    <!-- Setup: gh missing, signed out, or no GitHub remote -->
    <div v-if="!gh" class="gh-loading gh-fill"><LoaderCircle :size="16" class="spin" /> Checking the GitHub CLI…</div>

    <div v-else-if="!gh.installed" class="empty">
      <span class="glyph"><Download :size="22" /></span>
      <h2>Connect GitHub with the GitHub CLI</h2>
      <p>Ledgeling uses your own <code>gh</code>: your sign-in, your access, every repository and board you can reach. Nothing goes through Ledgeling.</p>
      <button v-if="ghTool?.installer" class="primary-button" @click="install"><Download :size="14" /> Install the GitHub CLI</button>
      <button v-else class="primary-button" @click="openOnGitHub('https://cli.github.com/')"><ExternalLink :size="14" /> Get the GitHub CLI</button>
      <button class="ghost" :disabled="checking" @click="loadTools(true); refreshGh()"><RefreshCw :size="13" :class="{ spin: checking }" /> I’ve installed it</button>
    </div>

    <div v-else-if="!gh.signedIn" class="empty">
      <span class="glyph"><LogIn :size="22" /></span>
      <h2>Sign in to GitHub</h2>
      <p>Sign in once in your browser. The GitHub CLI keeps the sign-in in your system keychain, and your terminals can use it too.</p>
      <button class="primary-button" @click="signIn"><LogIn :size="14" /> Sign in with the browser</button>
      <button class="ghost" :disabled="checking" @click="refreshGh().then(refreshLists)"><RefreshCw :size="13" :class="{ spin: checking }" /> I’ve signed in</button>
      <p class="muted small">Runs <code>gh auth login</code> in a terminal you can watch.</p>
    </div>

    <div v-else-if="!repo" class="empty">
      <span class="glyph"><TriangleAlert :size="22" /></span>
      <h2>No GitHub repository here</h2>
      <p>{{ gh.problem ?? 'This worktree has no remote on GitHub.' }}</p>
      <p class="muted small">Signed in as {{ gh.account }}. Add a GitHub remote with <code>git remote add origin …</code>, then check again.</p>
      <button class="ghost" :disabled="checking" @click="refreshGh().then(refreshLists)"><RefreshCw :size="13" :class="{ spin: checking }" /> Check again</button>
    </div>

    <template v-else>
      <header class="gh-top">
        <button class="bare gh-repo" :title="`Open ${repo.nameWithOwner} on GitHub`" @click="openOnGitHub(repo.url)">
          <span class="muted">{{ repo.owner }} /</span> <strong>{{ repo.name }}</strong>
        </button>
        <nav class="gh-sections" role="tablist" aria-label="GitHub">
          <button v-for="s in sections" :key="s.id" role="tab" :aria-selected="section === s.id" :title="s.label" @click="pickSection(s.id)">
            <component :is="s.icon" :size="13" /><span class="gh-section-label">{{ s.label }}</span>
            <span v-if="s.count" class="gh-count">{{ s.count > 99 ? '99+' : s.count }}</span>
          </button>
        </nav>
        <span class="grow" />
        <Menu :items="newItems" title="Create"><Plus :size="14" /></Menu>
        <button class="icon" title="Reload" :disabled="checking" @click="refreshGh().then(refreshLists)"><RefreshCw :size="13" :class="{ spin: checking || prs.loading.value || issues.loading.value }" /></button>
        <span class="gh-account" :title="`Signed in to GitHub as ${gh.account}`"><Avatar :login="gh.account" :size="22" /></span>
      </header>

      <div v-if="ghError" class="banner error gh-banner" role="alert"><p>{{ ghError }}</p><button class="icon" title="Dismiss" @click="dismissError"><X :size="13" /></button></div>

      <button v-if="branchPr && section !== 'boards'" class="bare gh-branch-strip" :class="prState(branchPr)" @click="show('pr', branchPr.number)">
        <GitBranch :size="12" /><span class="mono">{{ branch }}</span>
        <span class="muted">→</span>
        <strong>#{{ branchPr.number }}</strong><span class="gh-strip-title">{{ branchPr.title }}</span>
        <span class="grow" />
        <CircleX v-if="branchChecks.state === 'fail'" :size="13" class="gh-check fail" />
        <LoaderCircle v-else-if="branchChecks.state === 'pending'" :size="13" class="gh-check pending" />
        <CircleCheck v-else-if="branchChecks.state === 'pass'" :size="13" class="gh-check pass" />
        <span class="muted small">{{ rollupText(branchChecks) }}</span>
      </button>
      <div v-else-if="featureBranch && section === 'prs' && !composing" class="gh-branch-strip quiet">
        <GitBranch :size="12" /><span class="mono">{{ branch }}</span><span class="muted">has no pull request yet</span>
        <span class="grow" />
        <button class="primary-button" @click="startPr"><GitPullRequestArrow :size="13" /> Create pull request</button>
      </div>

      <BoardView v-if="section === 'boards'" />
      <div v-else class="gh-split" :class="{ split, detail: showingDetail }">
        <ItemList v-if="split || !showingDetail" :key="section" :kind="section === 'prs' ? 'pr' : 'issue'" />
        <div v-if="split || showingDetail" class="gh-pane">
          <NewIssue v-if="composing === 'issue'" />
          <NewPr v-else-if="composing === 'pr'" />
          <PrDetail v-else-if="open?.kind === 'pr'" :key="`pr${open.number}`" :number="open.number" :back="!split" />
          <IssueDetail v-else-if="open?.kind === 'issue'" :key="`i${open.number}`" :number="open.number" :back="!split" />
          <div v-else class="empty">
            <span class="glyph"><component :is="section === 'prs' ? GitPullRequest : CircleDot" :size="22" /></span>
            <h2>{{ section === 'prs' ? 'Pick a pull request' : 'Pick an issue' }}</h2>
            <p>Read it, discuss it, {{ section === 'prs' ? 'review and merge it, or check it out into its own worktree' : 'and start work on it in a worktree of its own' }}. ↑ and ↓ move through the list.</p>
          </div>
        </div>
      </div>
    </template>
  </section>
</template>

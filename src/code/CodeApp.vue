<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, reactive, ref, watch, type Ref } from 'vue'
import { Bot, ChevronDown, ChevronLeft, CircleCheck, CircleX, Columns2, FileCode2, FolderGit2, FolderOpen, FolderTree, GitBranch, GitCompareArrows, GitPullRequest, LoaderCircle, Maximize2, Minimize2, Minus, PanelLeftClose, PanelLeftOpen, Pin, Play, Rows2, Settings2, SquareTerminal, X } from '@lucide/vue'
import PetFace from '../assistant/PetFace.vue'
import WorktreeSwitcher from './components/WorktreeSwitcher.vue'
import ConfirmDialog from '../terminal/ConfirmDialog.vue'
import { activityOf, changeCount, code, codeState, current, currentPet, focus, loaded, narrowTabs, native, problem, refreshState, start, status, tab, wide } from './state'
import type { MainView, Tab } from './types'
import { openAntigravity, openClaude, openShell } from '../terminal/panes'
import { startNotes, unreadNotes } from './petNotes'
import { splitHint } from '../terminal/shortcuts'
import { has } from './tools'
import { branchPr, show as showOnGitHub } from './github/store'
import { prState, rollup } from './github/format'

// Each panel loads the first time it is shown; Monaco and xterm stay out of
// the window until the Editor, Changes, Run or Terminal view asks.
const panels: Record<Tab, unknown> = {
  trees: defineAsyncComponent(() => import('./components/FocusSidebar.vue')),
  files: defineAsyncComponent(() => import('./panels/FilesPanel.vue')),
  editor: defineAsyncComponent(() => import('./panels/EditorPanel.vue')),
  changes: defineAsyncComponent(() => import('./panels/ChangesPanel.vue')),
  github: defineAsyncComponent(() => import('./panels/GitHubPanel.vue')),
  worktrees: defineAsyncComponent(() => import('./panels/WorktreesPanel.vue')),
  run: defineAsyncComponent(() => import('./panels/RunPanel.vue')),
  terminal: defineAsyncComponent(() => import('./panels/TerminalPanel.vue')),
  chat: defineAsyncComponent(() => import('./panels/ChatPanel.vue')),
}
interface Item { id: Tab; label: string; icon: unknown; key: string }
/** The narrow window's single row of tabs. */
const topTabs: Item[] = [
  { id: 'files', label: 'Files', icon: FolderTree, key: '1' },
  { id: 'editor', label: 'Editor', icon: FileCode2, key: '2' },
  { id: 'changes', label: 'Changes', icon: GitCompareArrows, key: '3' },
  { id: 'worktrees', label: 'Worktrees', icon: FolderGit2, key: '4' },
  { id: 'run', label: 'Run', icon: Play, key: '5' },
  { id: 'terminal', label: 'Terminal', icon: SquareTerminal, key: '6' },
  { id: 'chat', label: 'Chat', icon: Bot, key: '7' },
  { id: 'github', label: 'GitHub', icon: GitPullRequest, key: '8' },
]
/** The wide layout's icon bar: views for the side panel, as in VS Code. */
const railTabs = computed<Item[]>(() => [
  { id: 'trees', label: 'Worktrees, with their files and changes', icon: GitBranch, key: '1' },
  { id: 'run', label: 'Run', icon: Play, key: '2' },
  { id: 'chat', label: currentPet.value ? `${currentPet.value.name}, on this worktree` : 'Chat', icon: Bot, key: '3' },
])
const railBottom: Item = { id: 'worktrees', label: 'Manage worktrees and add-ons', icon: Settings2, key: '' }
const mainTabs: (Item & { id: MainView })[] = [
  { id: 'terminal', label: 'Terminals', icon: SquareTerminal, key: '4' },
  { id: 'editor', label: 'Editor', icon: FileCode2, key: '5' },
  { id: 'changes', label: 'Changes', icon: GitCompareArrows, key: '6' },
  { id: 'github', label: 'GitHub', icon: GitPullRequest, key: '7' },
]
const isMain = (id: Tab): id is MainView => id === 'editor' || id === 'terminal' || id === 'changes' || id === 'github'

const mac = navigator.userAgent.includes('Mac')
const switching = ref(false)

/** Per-viewer layout conveniences; the page works the same without storage. */
function remembered<T>(key: string, fallback: T): Ref<T> {
  let initial = fallback
  try { const text = localStorage.getItem(`ledgeling.code.${key}`); if (text !== null) initial = JSON.parse(text) as T } catch { /* defaults */ }
  const value = ref(initial) as Ref<T>
  watch(() => value.value, next => { try { localStorage.setItem(`ledgeling.code.${key}`, JSON.stringify(next)) } catch { /* not kept */ } })
  return value
}
const sideOpen = remembered('sideOpen', true)
const sideWidth = remembered('sideWidth', 300)
const sideView = remembered<Tab>('sideView', 'trees')
const mainView = remembered<MainView>('mainView', 'terminal')

/** Every "show me X" in the window sets `tab`; the wide layout sends it to
 *  the main area or the side panel, opening the panel if it was hidden. */
watch(tab, t => {
  if (isMain(t)) mainView.value = t
  else { sideView.value = t; sideOpen.value = true }
})
// The worktree list only exists in the wide layout; narrow, it is the switcher.
watch(wide, isWide => { if (!isWide && tab.value === 'trees') tab.value = 'worktrees' })

/** What each panel is showing as right now, if anything. */
function place(id: Tab): 'side' | 'main' | null {
  if (!wide.value) return tab.value === id || (id === 'worktrees' && tab.value === 'trees') ? 'main' : null
  if (isMain(id)) return mainView.value === id ? 'main' : null
  return sideOpen.value && sideView.value === id ? 'side' : null
}
/** Tabs keep their state once opened: terminals keep running, files stay open. */
const visited = reactive(new Set<Tab>())
watch(() => (Object.keys(panels) as Tab[]).filter(id => place(id)), shown => shown.forEach(id => visited.add(id)), { immediate: true })

/** An icon on the bar opens its view; the one already open hides the panel. */
function railClick(id: Tab) {
  if (sideOpen.value && sideView.value === id) sideOpen.value = false
  else tab.value = id
}
// Files opens from a worktree in the list, so it counts as that list's view.
const railOn = (id: Tab) => sideOpen.value && (sideView.value === id || (id === 'trees' && sideView.value === 'files'))
/** The side view shows a way back to the worktree list it was opened from. */
const backToTrees = computed(() => wide.value && sideOpen.value && sideView.value === 'files')

const activity = computed(() => current.value ? activityOf(current.value.worktree.path) : null)
const running = computed(() => codeState.value.sessions.some(s => s.kind === 'run' && s.alive && s.worktree === current.value?.worktree.path))
const branch = computed(() => status.value?.branch ?? current.value?.worktree.branch ?? 'detached')
/** The branch's pull request, once the GitHub view has been opened. */
const prChecks = computed(() => rollup(branchPr.value?.statusCheckRollup).state)
function openBranchPr() {
  if (!branchPr.value) return
  showOnGitHub('pr', branchPr.value.number)
  tab.value = 'github'
}

async function addProject() {
  try { await code('code_add_project'); await refreshState() } catch { /* shown in the banner */ }
}
async function windowAction(action: 'pin' | 'focus' | 'minimize' | 'close') {
  try {
    const on = await code<boolean>('code_window', { action })
    if (action === 'pin') codeState.value = { ...codeState.value, pinned: on }
    if (action === 'focus') {
      codeState.value = { ...codeState.value, focus: on }
      // Focus mode is about the terminals: put them in front.
      if (on) tab.value = 'terminal'
    }
  } catch { /* banner */ }
}

function dragSide(event: PointerEvent) {
  const handle = event.currentTarget as HTMLElement
  handle.setPointerCapture(event.pointerId)
  const start = event.clientX
  const initial = sideWidth.value
  const move = (e: PointerEvent) => { sideWidth.value = Math.round(Math.min(Math.max(initial + e.clientX - start, 220), Math.max(260, window.innerWidth * 0.5))) }
  const done = () => { handle.removeEventListener('pointermove', move); handle.removeEventListener('pointerup', done) }
  handle.addEventListener('pointermove', move)
  handle.addEventListener('pointerup', done)
}

function onKey(event: KeyboardEvent) {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
  // Cmd/Ctrl+B shows and hides the side panel, as in VS Code.
  if (wide.value && event.key.toLowerCase() === 'b') { event.preventDefault(); sideOpen.value = !sideOpen.value; return }
  const target = (wide.value ? [...railTabs.value, ...mainTabs] : topTabs).find(t => t.key === event.key)
  if (!target) return
  event.preventDefault()
  if (wide.value && !isMain(target.id)) railClick(target.id)
  else tab.value = target.id
}

let stop = () => {}
onMounted(() => {
  stop = start()
  startNotes()
  window.addEventListener('keydown', onKey)
})
onUnmounted(() => {
  stop()
  window.removeEventListener('keydown', onKey)
})
</script>

<template>
  <div class="code-shell" :class="{ focused: focus }">
    <header class="code-header" :class="{ mac }" data-tauri-drag-region>
      <button v-if="current" class="switch-button" :aria-expanded="switching" aria-haspopup="dialog" title="Switch worktree" @click="switching = !switching">
        <PetFace :pet="currentPet ? { packId: currentPet.packId, name: currentPet.name } : null" :pet-id="currentPet?.id ?? null" :size="28" />
        <span class="switch-text">
          <strong>{{ current.worktree.name }}</strong>
          <small><FolderGit2 :size="11" /> {{ branch }}<template v-if="currentPet"> · {{ currentPet.name }}</template><template v-else> · no pet</template></small>
        </span>
        <ChevronDown :size="14" class="muted" />
      </button>
      <span class="spacer" data-tauri-drag-region />
      <span class="chips" data-tauri-drag-region>
        <span v-if="activity" class="chip" :class="activity.tone"><span class="dot" :class="activity.tone === 'idle' ? 'idle' : activity.tone === 'live' ? 'live' : 'busy'" />{{ activity.label }}</span>
        <span v-else-if="running" class="chip live"><span class="dot live" />Running</span>
        <button v-if="branchPr" class="chip gh-pr-chip" :class="prState(branchPr)" :title="`#${branchPr.number} ${branchPr.title}`" @click="openBranchPr">
          <CircleX v-if="prChecks === 'fail'" :size="11" /><LoaderCircle v-else-if="prChecks === 'pending'" :size="11" /><CircleCheck v-else-if="prChecks === 'pass'" :size="11" /><GitPullRequest v-else :size="11" />#{{ branchPr.number }}
        </button>
      </span>
      <button v-if="current" class="icon" :class="{ on: focus }" :aria-pressed="focus" :title="focus ? 'Leave focus mode' : 'Focus mode: full screen, terminals in front'" @click="windowAction('focus')">
        <Minimize2 v-if="focus" :size="14" /><Maximize2 v-else :size="14" />
      </button>
      <button class="icon" :class="{ on: codeState.pinned }" :aria-pressed="codeState.pinned" :title="codeState.pinned ? 'Unpin: let other windows cover this one' : 'Keep on top of other windows'" @click="windowAction('pin')"><Pin :size="14" /></button>
      <template v-if="!mac">
        <button class="icon" title="Minimize" @click="windowAction('minimize')"><Minus :size="14" /></button>
        <button class="icon" title="Close" @click="windowAction('close')"><X :size="14" /></button>
      </template>
    </header>

    <WorktreeSwitcher v-if="switching" @close="switching = false" @add-project="addProject" />
    <ConfirmDialog />

    <div v-if="current" class="workspace" :class="wide ? ['wide', { collapsed: !sideOpen }] : 'top'" :style="{ '--side-w': `${sideWidth}px` }">
      <nav v-if="wide" class="rail" role="tablist" aria-label="Side panel views" aria-orientation="vertical">
        <button v-for="item in railTabs" :key="item.id" role="tab" :aria-selected="railOn(item.id)" :title="`${item.label} (${mac ? '⌘' : 'Ctrl+'}${item.key})`" @click="railClick(item.id)">
          <PetFace v-if="item.id === 'chat' && currentPet" :pet="{ packId: currentPet.packId, name: currentPet.name }" :size="26" />
          <component :is="item.icon" v-else :size="18" />
          <span v-if="item.id === 'trees' && changeCount" class="count">{{ changeCount > 99 ? '99+' : changeCount }}</span>
          <span v-if="item.id === 'run' && running" class="tab-dot" />
          <span v-if="item.id === 'chat' && unreadNotes && !railOn('chat')" class="tab-dot ask" />
        </button>
        <span class="grow" />
        <button role="tab" :aria-selected="railOn(railBottom.id)" :title="railBottom.label" @click="railClick(railBottom.id)"><component :is="railBottom.icon" :size="18" /></button>
        <button :title="`${sideOpen ? 'Hide' : 'Show'} the side panel (${mac ? '⌘' : 'Ctrl+'}B)`" @click="sideOpen = !sideOpen">
          <PanelLeftClose v-if="sideOpen" :size="18" /><PanelLeftOpen v-else :size="18" />
        </button>
      </nav>
      <nav v-else class="code-tabs" :class="{ narrow: narrowTabs }" role="tablist" aria-label="Coding views">
        <button v-for="item in topTabs" :key="item.id" role="tab" :aria-selected="place(item.id) === 'main'" :title="`${item.label} (${mac ? '⌘' : 'Ctrl+'}${item.key})`" @click="tab = item.id">
          <component :is="item.icon" :size="15" />
          <span class="tab-label">{{ item.label }}</span>
          <span v-if="item.id === 'changes' && changeCount" class="count">{{ changeCount > 99 ? '99+' : changeCount }}</span>
          <span v-if="item.id === 'run' && running" class="tab-dot" />
        </button>
      </nav>

      <div v-if="backToTrees" class="side-back">
        <button class="bare" title="Back to the worktree list" @click="tab = 'trees'"><ChevronLeft :size="14" /> Worktrees</button>
        <span class="muted">/</span><strong>{{ current.worktree.name }}</strong>
      </div>
      <div v-if="wide && sideOpen" class="col-resizer" role="separator" aria-orientation="vertical" title="Drag to resize" @pointerdown.prevent="dragSide" />

      <div class="main-head">
        <div v-if="wide" class="main-tabs" role="tablist" aria-label="Main area">
          <button v-for="item in mainTabs" :key="item.id" role="tab" :aria-selected="mainView === item.id" :title="`${item.label} (${mac ? '⌘' : 'Ctrl+'}${item.key})`" @click="tab = item.id">
            <component :is="item.icon" :size="13" /> {{ item.label }}
            <span v-if="item.id === 'changes' && changeCount" class="count">{{ changeCount > 99 ? '99+' : changeCount }}</span>
          </button>
          <template v-if="mainView === 'terminal'">
            <span class="grow" />
            <button class="ghost" :disabled="!has('claude')" :title="has('claude') ? 'Start Claude Code beside the focused terminal' : 'Claude Code is not installed. See Add-ons.'" @click="openClaude"><Bot :size="13" /> Claude Code</button>
            <button v-if="has('antigravity')" class="ghost" title="Start Antigravity beside the focused terminal" @click="openAntigravity"><Bot :size="13" /> Antigravity</button>
            <button class="icon" :title="`New terminal on the right (${splitHint('row')})`" @click="openShell('row')"><Columns2 :size="14" /></button>
            <button class="icon" :title="`New terminal below (${splitHint('column')})`" @click="openShell('column')"><Rows2 :size="14" /></button>
          </template>
        </div>
        <div v-if="problem" class="banner error" role="alert"><p>{{ problem }}</p><button class="icon" title="Dismiss" @click="problem = ''"><X :size="13" /></button></div>
      </div>

      <template v-for="id in (Object.keys(panels) as Tab[])" :key="id">
        <component
          :is="panels[id]" v-if="visited.has(id)" v-show="place(id)" :active="!!place(id)"
          class="panel" :class="[place(id) ?? '', { below: backToTrees && id === 'files' }]" role="tabpanel" @collapse="sideOpen = false"
        />
      </template>
    </div>

    <div v-else-if="loaded || !native" class="empty">
      <span class="glyph"><FolderOpen :size="22" /></span>
      <h2>Pick a project to start coding</h2>
      <p>Choose a folder with your code. Ledgeling keeps it next to your browser so you can watch changes as they happen.</p>
      <ul class="tour">
        <li><FolderTree :size="14" /> Files and Editor to read and change code</li>
        <li><GitCompareArrows :size="14" /> Changes shows every edit, live</li>
        <li><FolderGit2 :size="14" /> Worktrees give each pet its own copy to work in</li>
        <li><Play :size="14" /> Run starts your project with one button</li>
        <li><SquareTerminal :size="14" /> Terminal runs Claude Code and your shell, split as you like</li>
        <li><Maximize2 :size="14" /> Focus mode fills the screen with your worktrees and terminals</li>
      </ul>
      <button class="primary-button" :disabled="!native" @click="addProject"><FolderOpen :size="14" /> Choose project folder</button>
      <p v-if="problem" class="banner error">{{ problem }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { Bot, ChevronDown, FileCode2, FolderGit2, FolderOpen, FolderTree, GitCompareArrows, Minus, Pin, Play, SquareTerminal, X } from '@lucide/vue'
import PetFace from '../assistant/PetFace.vue'
import WorktreeSwitcher from './components/WorktreeSwitcher.vue'
import { activityOf, changeCount, code, codeState, current, currentPet, loaded, native, problem, refreshState, start, status, tab } from './state'
import type { Tab } from './types'

// Each panel loads the first time its tab is opened; Monaco and xterm stay
// out of the window until the Editor, Changes, Run or Terminal tab asks.
const panels = {
  files: defineAsyncComponent(() => import('./panels/FilesPanel.vue')),
  editor: defineAsyncComponent(() => import('./panels/EditorPanel.vue')),
  changes: defineAsyncComponent(() => import('./panels/ChangesPanel.vue')),
  worktrees: defineAsyncComponent(() => import('./panels/WorktreesPanel.vue')),
  run: defineAsyncComponent(() => import('./panels/RunPanel.vue')),
  terminal: defineAsyncComponent(() => import('./panels/TerminalPanel.vue')),
  chat: defineAsyncComponent(() => import('./panels/ChatPanel.vue')),
}
const tabs: { id: Tab; label: string; icon: unknown; key: string }[] = [
  { id: 'files', label: 'Files', icon: FolderTree, key: '1' },
  { id: 'editor', label: 'Editor', icon: FileCode2, key: '2' },
  { id: 'changes', label: 'Changes', icon: GitCompareArrows, key: '3' },
  { id: 'worktrees', label: 'Worktrees', icon: FolderGit2, key: '4' },
  { id: 'run', label: 'Run', icon: Play, key: '5' },
  { id: 'terminal', label: 'Terminal', icon: SquareTerminal, key: '6' },
  { id: 'chat', label: 'Chat', icon: Bot, key: '7' },
]
/** Tabs keep their state once opened: terminals keep running, files stay open. */
const visited = reactive(new Set<Tab>(['files']))
watch(tab, t => visited.add(t), { immediate: true })

const mac = navigator.userAgent.includes('Mac')
const switching = ref(false)
const width = ref(window.innerWidth)
// Seven labelled tabs need about 620px; narrower, only the open tab keeps its label.
const narrow = computed(() => width.value < 620)
const onResize = () => { width.value = window.innerWidth }

const activity = computed(() => current.value ? activityOf(current.value.worktree.path) : null)
const running = computed(() => codeState.value.sessions.some(s => s.kind === 'run' && s.alive && s.worktree === current.value?.worktree.path))
const branch = computed(() => status.value?.branch ?? current.value?.worktree.branch ?? 'detached')

async function addProject() {
  try { await code('code_add_project'); await refreshState() } catch { /* shown in the banner */ }
}
async function windowAction(action: 'pin' | 'minimize' | 'close') {
  try {
    const pinned = await code<boolean>('code_window', { action })
    if (action === 'pin') codeState.value = { ...codeState.value, pinned }
  } catch { /* banner */ }
}

function onKey(event: KeyboardEvent) {
  if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
  const target = tabs.find(t => t.key === event.key)
  if (target) { event.preventDefault(); tab.value = target.id }
}

let stop = () => {}
onMounted(() => {
  stop = start()
  window.addEventListener('keydown', onKey)
  window.addEventListener('resize', onResize)
})
onUnmounted(() => {
  stop()
  window.removeEventListener('keydown', onKey)
  window.removeEventListener('resize', onResize)
})
</script>

<template>
  <div class="code-shell">
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
      </span>
      <button class="icon" :class="{ on: codeState.pinned }" :aria-pressed="codeState.pinned" :title="codeState.pinned ? 'Unpin: let other windows cover this one' : 'Keep on top of other windows'" @click="windowAction('pin')"><Pin :size="14" /></button>
      <template v-if="!mac">
        <button class="icon" title="Minimize" @click="windowAction('minimize')"><Minus :size="14" /></button>
        <button class="icon" title="Close" @click="windowAction('close')"><X :size="14" /></button>
      </template>
    </header>

    <WorktreeSwitcher v-if="switching" @close="switching = false" @add-project="addProject" />

    <template v-if="current">
      <nav class="code-tabs" :class="{ narrow }" role="tablist" aria-label="Coding views">
        <button v-for="item in tabs" :key="item.id" role="tab" :aria-selected="tab === item.id" :title="`${item.label} (${mac ? '⌘' : 'Ctrl+'}${item.key})`" @click="tab = item.id">
          <component :is="item.icon" :size="15" />
          <span class="tab-label">{{ item.label }}</span>
          <span v-if="item.id === 'changes' && changeCount" class="count">{{ changeCount > 99 ? '99+' : changeCount }}</span>
          <span v-if="item.id === 'run' && running" class="tab-dot" />
        </button>
      </nav>
      <div v-if="problem" class="banner error" role="alert"><p>{{ problem }}</p><button class="icon" title="Dismiss" @click="problem = ''"><X :size="13" /></button></div>
      <main class="code-main">
        <template v-for="item in tabs" :key="item.id">
          <component :is="panels[item.id]" v-if="visited.has(item.id)" v-show="tab === item.id" :active="tab === item.id" class="panel" role="tabpanel" />
        </template>
      </main>
    </template>

    <div v-else-if="loaded || !native" class="empty">
      <span class="glyph"><FolderOpen :size="22" /></span>
      <h2>Pick a project to start coding</h2>
      <p>Choose a folder with your code. Ledgeling keeps it next to your browser so you can watch changes as they happen.</p>
      <ul class="tour">
        <li><FolderTree :size="14" /> Files and Editor to read and change code</li>
        <li><GitCompareArrows :size="14" /> Changes shows every edit, live</li>
        <li><FolderGit2 :size="14" /> Worktrees give each pet its own copy to work in</li>
        <li><Play :size="14" /> Run starts your project with one button</li>
        <li><SquareTerminal :size="14" /> Terminal runs Claude Code and your shell</li>
      </ul>
      <button class="primary-button" :disabled="!native" @click="addProject"><FolderOpen :size="14" /> Choose project folder</button>
      <p v-if="problem" class="banner error">{{ problem }}</p>
    </div>
  </div>
</template>

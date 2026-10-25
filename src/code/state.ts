import { computed, ref, shallowRef, watch } from 'vue'
import { Channel, invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { pets, refresh, refreshPacks } from '../state'
import type { CodeState, Changes, GitStatus, ProjectView, Tab, WorktreeView } from './types'

/** Rust owns the truth; this is the coding window's copy of it. */
export const native = isTauri()
export const codeState = ref<CodeState>({ projects: [], selected: null, pinned: false, focus: false, sessions: [], notes: 0 })
export const loaded = ref(false)
export const problem = ref('')
export const tab = ref<Tab>('files')

export async function code<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!native) throw new Error('Open Ledgeling to use coding mode.')
  try {
    return await invoke<T>(command, args)
  } catch (e) {
    problem.value = String(e)
    throw e
  }
}

export async function refreshState() {
  if (!native) return
  try {
    codeState.value = await invoke<CodeState>('code_state')
    loaded.value = true
  } catch (e) {
    problem.value = String(e)
  }
}

/** Focus mode: the window fills its screen, with the terminals in front. */
export const focus = computed(() => codeState.value.focus)

const width = ref(window.innerWidth)
window.addEventListener('resize', () => { width.value = window.innerWidth })
/** Wide windows are laid out like VS Code: an icon bar, a collapsible side
 *  panel, and the editor or terminals in the main area. Narrow ones keep a
 *  single row of tabs along the top. */
export const WIDE_FROM = 760
export const wide = computed(() => focus.value || width.value >= WIDE_FROM)
export const narrowTabs = computed(() => !wide.value && width.value < 620)

export const worktrees = computed(() => codeState.value.projects.flatMap(p => p.worktrees.map(w => ({ project: p, worktree: w }))))

/** The worktree every tab shows. Nothing in the window ever mixes two. */
export const current = computed<{ project: ProjectView; worktree: WorktreeView } | null>(() => {
  const all = worktrees.value
  return all.find(e => e.worktree.path === codeState.value.selected) ?? all[0] ?? null
})
export const root = computed(() => current.value?.worktree.path ?? null)
export const currentPet = computed(() => {
  const id = current.value?.worktree.petId
  return id == null ? null : pets.value.find(p => p.id === id) ?? null
})

export function petFor(worktree: WorktreeView) {
  return worktree.petId == null ? null : pets.value.find(p => p.id === worktree.petId) ?? null
}

export async function select(path: string) {
  codeState.value = { ...codeState.value, selected: path }
  await code('code_select', { worktree: path })
}

/** Git status for the worktree on screen, refreshed whenever files move. */
export const status = shallowRef<GitStatus | null>(null)
let statusTimer: ReturnType<typeof setTimeout> | undefined
export function refreshStatus(delay = 0) {
  clearTimeout(statusTimer)
  statusTimer = setTimeout(async () => {
    const path = root.value
    if (!path) { status.value = null; return }
    try {
      const next = await invoke<GitStatus>('code_status', { worktree: path })
      if (root.value === path) status.value = next
    } catch { status.value = null }
  }, delay)
}
export const changeCount = computed(() => status.value?.files.length ?? 0)

/** Bumped on every batch of file changes, with the paths that changed. */
export const fileChanges = shallowRef<Changes>({ paths: [], git: false })
export const fileTick = ref(0)

function watchRoot(path: string | null) {
  if (!path || !native) return
  const channel = new Channel<Changes>()
  channel.onmessage = changes => {
    if (root.value !== path) return
    fileChanges.value = changes
    fileTick.value++
    refreshStatus(120)
  }
  void invoke('code_watch', { worktree: path, channel }).catch(e => { problem.value = String(e) })
}

/** A file the Files or Changes tab asked the editor to open. */
export const openRequest = ref<{ path: string; line?: number; nonce: number } | null>(null)
export function openFile(path: string, line?: number) {
  openRequest.value = { path, line, nonce: Date.now() }
  tab.value = 'editor'
}

/** The Changes tab can be pointed at one file from elsewhere. */
export const diffRequest = ref<string | null>(null)
export function showDiff(path: string) {
  diffRequest.value = path
  tab.value = 'changes'
}

export function short(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path
}

export function start() {
  if (!native) return () => {}
  void refreshState()
  void refresh()
  void refreshPacks()
  // Pets come from the shared snapshot; they rarely change, so a slow poll.
  const timer = setInterval(() => { void refresh(); void refreshState() }, 3000)
  const offs = [
    listen('code:changed', () => void refreshState()),
    listen('packs:changed', () => void refreshPacks()),
  ]
  const stopWatch = watch(root, path => { status.value = null; watchRoot(path); refreshStatus() }, { immediate: true })
  return () => {
    clearInterval(timer)
    stopWatch()
    for (const off of offs) void off.then(stop => stop())
  }
}

export interface Activity { label: string; tone: 'ask' | 'busy' | 'live' | 'idle' }
/** One plain-words line for what is happening in a worktree right now. */
export function activityOf(path: string): Activity | null {
  const sessions = codeState.value.sessions.filter(s => s.worktree === path && s.alive)
  if (sessions.some(s => s.attention === 'needs_user')) return { label: 'Needs you', tone: 'ask' }
  if (sessions.some(s => s.attention === 'busy')) return { label: 'Agent working', tone: 'busy' }
  if (sessions.some(s => s.kind === 'run')) return { label: 'Running', tone: 'live' }
  if (sessions.some(s => s.kind === 'agent')) return { label: 'Agent idle', tone: 'idle' }
  return null
}

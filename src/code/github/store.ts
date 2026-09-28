import { computed, ref, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { code, root, status as gitStatus } from '../state'
import { localTarget } from './format'
import type { Board, Filter, GhStatus, Issue, IssueSummary, Kind, Pr, PrSummary, ProjectSummary, RepoMeta, Section } from './types'

/** Nothing here reaches GitHub until the GitHub view is first opened. */
export const started = ref(false)

export const gh = shallowRef<GhStatus | null>(null)
export const checking = ref(false)
export const repo = computed(() => gh.value?.repo ?? null)
export const ready = computed(() => !!gh.value?.signedIn && !!repo.value)
export const canBoards = computed(() => gh.value?.scopes.includes('project') || gh.value?.scopes.includes('read:project'))
export const canWrite = computed(() => ['ADMIN', 'MAINTAIN', 'WRITE', 'TRIAGE'].includes(repo.value?.viewerPermission ?? ''))

export const section = ref<Section>('prs')
/** What the detail pane shows; `null` shows the list alone. */
export const open = ref<{ kind: Kind; number: number } | null>(null)
export const composing = ref<'issue' | 'pr' | null>(null)
/** Survives the next worktree switch, so checking out a pull request keeps it open. */
export const carry = ref<{ kind: Kind; number: number } | null>(null)
export const ghError = ref('')

export async function ask<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const worktree = root.value
  if (!worktree) throw new Error('Pick a worktree first.')
  try {
    return await invoke<T>(command, { worktree, ...args })
  } catch (e) {
    ghError.value = String(e)
    throw e
  }
}

export async function refreshGh() {
  const worktree = root.value
  if (!worktree) return
  checking.value = true
  try {
    const next = await invoke<GhStatus>('code_gh_status', { worktree })
    if (root.value === worktree) gh.value = next
  } catch (e) {
    ghError.value = String(e)
  } finally {
    checking.value = false
  }
}

/** One list per kind, refetched when its filter or search changes. */
function lister<T>(kind: Kind) {
  const items = shallowRef<T[]>([])
  const filter = ref<Filter>('open')
  const search = ref('')
  const loading = ref(false)
  const loadedFor = ref('')
  let seq = 0
  async function load() {
    if (!ready.value) return
    const mine = ++seq
    loading.value = true
    try {
      const list = await ask<T[]>('code_gh_list', { kind, filter: filter.value, search: search.value })
      if (mine === seq) { items.value = list; loadedFor.value = root.value ?? '' }
    } catch { /* ghError */ } finally {
      if (mine === seq) loading.value = false
    }
  }
  return { items, filter, search, loading, loadedFor, load }
}
export const prs = lister<PrSummary>('pr')
export const issues = lister<IssueSummary>('issue')

/** The pull request for the branch on screen, shown in the window header. */
export const branchPr = shallowRef<PrSummary | null>(null)
export async function refreshBranchPr() {
  if (!ready.value) { branchPr.value = null; return }
  const worktree = root.value
  try {
    const found = await invoke<PrSummary | null>('code_gh_branch_pr', { worktree })
    if (root.value === worktree) branchPr.value = found
  } catch { branchPr.value = null }
}

export const meta = shallowRef<RepoMeta | null>(null)
export async function loadMeta(force = false) {
  if (meta.value && !force) return meta.value
  try { meta.value = await ask<RepoMeta>('code_gh_meta') } catch { /* ghError */ }
  return meta.value
}

export async function view<T extends Pr | Issue>(kind: Kind, number: number): Promise<T> {
  return ask<T>('code_gh_view', { kind, number })
}

export async function act(action: Record<string, unknown>) {
  const out = await ask<string>('code_gh_act', { action })
  void refreshLists()
  return out
}

export function refreshLists() {
  void prs.load()
  void issues.load()
  void refreshBranchPr()
}

/** Opens an issue or pull request in the detail pane, from anywhere. */
export function show(kind: Kind, number: number) {
  section.value = kind === 'pr' ? 'prs' : 'issues'
  open.value = { kind, number }
  composing.value = null
}

// ------------------------------------------------------------------ boards

export const projects = shallowRef<ProjectSummary[] | null>(null)
export const projectsLoading = ref(false)
export const boardId = ref<string | null>(null)
export const board = shallowRef<Board | null>(null)
export const boardLoading = ref(false)

export async function loadProjects() {
  projectsLoading.value = true
  try {
    projects.value = await ask<ProjectSummary[]>('code_gh_projects')
    const remembered = rememberedBoard()
    const pick = projects.value.find(p => p.id === remembered) ?? projects.value.find(p => p.linked) ?? projects.value[0] ?? null
    if (pick && boardId.value !== pick.id) boardId.value = pick.id
  } catch { projects.value = [] } finally { projectsLoading.value = false }
}

export async function loadBoard() {
  const id = boardId.value
  if (!id) { board.value = null; return }
  boardLoading.value = true
  try {
    const next = await ask<Board>('code_gh_board', { project: id })
    if (boardId.value === id) board.value = next
  } catch { /* ghError */ } finally { boardLoading.value = false }
}

export function boardAct(action: Record<string, unknown>) {
  return ask<string | null>('code_gh_board_act', { project: boardId.value, action })
}

/** Which board each repository showed last; a per-viewer convenience. */
function rememberedBoard(): string | null {
  try { return localStorage.getItem(`ledgeling.code.board.${repo.value?.nameWithOwner}`) } catch { return null }
}
watch(boardId, id => {
  try { if (id && repo.value) localStorage.setItem(`ledgeling.code.board.${repo.value.nameWithOwner}`, id) } catch { /* not kept */ }
  board.value = null
  void loadBoard()
})

// -------------------------------------------------------------- lifecycle

/** Runs once, when the GitHub view first opens; switching worktrees starts over. */
export function startGithub() {
  if (started.value) return
  started.value = true
  watch(root, async () => {
    const keep = carry.value
    carry.value = null
    gh.value = null
    branchPr.value = null
    meta.value = null
    projects.value = null
    boardId.value = null
    open.value = null
    prs.items.value = []
    issues.items.value = []
    await refreshGh()
    refreshLists()
    if (keep) show(keep.kind, keep.number)
  }, { immediate: true })
  // A new branch or a push can change which pull request the header shows.
  watch(() => gitStatus.value?.branch, (next, before) => { if (before !== undefined && next !== before) void refreshBranchPr() })
  // GitHub changes without telling us; a slow poll keeps the header honest.
  setInterval(() => { if (ready.value) void refreshBranchPr() }, 120_000)
}

export async function openOnGitHub(url: string) {
  await code('code_open_url', { url }).catch(() => {})
}

/** Links to this repository's issues and pull requests open here; the rest in the browser. */
export function follow(url: string) {
  const target = repo.value ? localTarget(url, repo.value.url) : null
  if (target) show(target.kind, target.number)
  else void openOnGitHub(url)
}

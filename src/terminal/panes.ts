import { reactive, watch } from 'vue'
import { ids, insert, keep, parse, type Direction, type Layout } from './layout'
import { backend, type SpawnOptions } from './backend'
import { adopt, close, spawn, terms } from './terminals'

/** The folder whose terminals are on screen. */
const root = { get value() { return backend().scope.value } }

/** Each worktree's split layout, kept in Rust so it outlives the window. */
export const layouts = reactive(new Map<string, Layout | null>())
/** The pane that last had the keyboard, per worktree: new panes open beside it. */
export const focused = reactive(new Map<string, number>())

let hint: { worktree: string; near: number | null; dir: Direction } | null = null
let ready: Promise<void> | null = null

const timers = new Map<string, ReturnType<typeof setTimeout>>()
function save(worktree: string) {
  clearTimeout(timers.get(worktree))
  timers.set(worktree, setTimeout(() => {
    void backend().saveLayout(worktree, layouts.get(worktree) ?? null).catch(() => {})
  }, 300))
}

export function setLayout(worktree: string, layout: Layout | null) {
  layouts.set(worktree, layout)
  save(worktree)
}

/** Installers belong to no worktree; they join whichever grid is on screen. */
function placedElsewhere(id: number, worktree: string) {
  for (const [tree, layout] of layouts) if (tree !== worktree && ids(layout).includes(id)) return true
  return false
}

/** Match a worktree's layout to the terminals that exist: closed ones leave,
 *  new ones open beside the focused pane (or where `spawnPane` asked). */
export function reconcile(worktree: string) {
  const mine = [...terms.values()].filter(t =>
    t.worktree === worktree || (!t.worktree && worktree === root.value && !placedElsewhere(t.id, worktree)))
  const present = new Set(mine.map(t => t.id))
  const before = layouts.get(worktree) ?? null
  let layout = keep(before, id => present.has(id))
  const known = new Set(ids(layout))
  for (const t of mine) {
    if (known.has(t.id)) continue
    const placed = hint?.worktree === worktree ? hint : null
    const near = placed ? placed.near : focused.get(worktree) ?? null
    layout = insert(layout, near, t.id, placed?.dir ?? 'row')
    if (placed) hint = null
    focused.set(worktree, t.id)
  }
  if (JSON.stringify(layout) !== JSON.stringify(before)) setLayout(worktree, layout)
  const current = focused.get(worktree)
  if (current === undefined || !present.has(current)) {
    const first = ids(layout)[0]
    if (first === undefined) focused.delete(worktree)
    else focused.set(worktree, first)
  }
}

function reconcileAll() {
  const trees = new Set<string>([...layouts.keys()])
  for (const t of terms.values()) trees.add(t.worktree || root.value || '')
  trees.delete('')
  for (const tree of trees) reconcile(tree)
}

/** Stored layouts first, then running terminals, so a reopened window does
 *  not prune layouts before it has adopted the sessions they name. */
export function initPanes() {
  ready ??= (async () => {
    try {
      const stored = await backend().layouts()
      for (const [tree, value] of Object.entries(stored)) layouts.set(tree, parse(value))
    } catch { /* start empty */ }
    await adopt()
    reconcileAll()
    watch(() => [...terms.keys()].join(','), reconcileAll)
    watch(() => backend().scope.value, reconcileAll)
  })()
  return ready
}

/** Start a terminal in a new pane beside `near`. */
export async function spawnPane(options: SpawnOptions, near: number | null, dir: Direction = 'row') {
  await initPanes()
  hint = { worktree: options.worktree || root.value || '', near, dir }
  const entry = await spawn(options)
  if (!entry) hint = null
  backend().changed()
  return entry
}

export async function closePane(id: number) {
  await close(id)
  backend().changed()
}

/** A shell beside the focused terminal of the worktree on screen. */
export function openShell(dir: Direction, near: number | null = focused.get(root.value ?? '') ?? null) {
  const worktree = root.value
  if (!worktree) return
  void spawnPane({ worktree, kind: 'shell', title: nextTitle(worktree, 'Shell') }, near, dir)
}

/** An agent the user has installed, beside the focused terminal. */
export function openAgent(agent: { name: string; command: string }) {
  const worktree = root.value
  if (!worktree) return
  void spawnPane({ worktree, kind: 'agent', command: agent.command, title: nextTitle(worktree, agent.name) }, focused.get(worktree) ?? null)
}

/** The user's own `claude`, beside the focused terminal. */
export const openClaude = () => openAgent({ name: 'Claude Code', command: 'claude' })

/** "Shell", then "Shell 2" and so on, so panes can be told apart. */
export function nextTitle(worktree: string, base: string) {
  const taken = new Set([...terms.values()].filter(t => t.worktree === worktree).map(t => t.title))
  if (!taken.has(base)) return base
  let n = 2
  while (taken.has(`${base} ${n}`)) n++
  return `${base} ${n}`
}

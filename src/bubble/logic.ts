import { terminal, type ApprovalSummary, type Task } from '../assistant/types'

/** What `show_pet_bubble` sends when the bubble opens on a pet. */
export interface BubbleShow {
  session: number
  petId: number
  name: string
  /** `none`, `ollama`, `codex`, `claude`, `apple`, or `model` for any other API. */
  runtime: string
  model: string
  folder: string
  /** Developer tools are on, so the bubble offers coding mode. */
  developer?: boolean
  /** First run: the bubble opens as the tour until it is finished or skipped. */
  tour?: boolean
  tailX: number | null
  tailUp: boolean
}

/** Everything else the backend tells the bubble arrives as one of these. */
export type BubbleUpdate =
  | Task
  | { petId?: number; folder?: string; models?: string[]; model?: string; message?: string }

/** Built-in actions, in menu order. The last one sends free-form text. */
export const ACTIONS = [
  { preset: 'sort_type', label: 'Sort by type' },
  { preset: 'sort_date', label: 'Sort by month' },
  { preset: 'rename', label: 'Rename with prefix' },
  { preset: 'find', label: 'Find files' },
  { preset: 'duplicates', label: 'Find duplicates' },
  { preset: '', label: 'My instructions' },
] as const

export type Mode = 'actions' | 'ask'

export interface Draft {
  mode: Mode
  action: number
  prompt: string
  allowCloud: boolean
  allowWeb: boolean
}

export const connected = (runtime: string) => runtime !== '' && runtime !== 'none'

/** What the current mode still needs before it could possibly run, or null.
 *  Saying so here beats letting the task start and fail a second later. At
 *  most one thing is ever missing, so the user is never handed a list of
 *  conditions. Mirrors `compose_request`, which has the final word. */
export function blocker(show: Pick<BubbleShow, 'runtime' | 'folder'>, draft: Draft): string | null {
  const text = draft.prompt.trim()
  if (draft.mode === 'ask') {
    if (!connected(show.runtime)) return 'Set up an AI provider to ask.'
    if (!text) return 'Type a question first.'
    return null
  }
  const custom = ACTIONS[draft.action]?.preset === ''
  // Apple's model answers in one turn and cannot act on files, so its Actions are the built-in ones.
  if (show.runtime === 'apple' && custom) return "Apple Intelligence can't change files. Pick a built-in action, or use Ask."
  if (!connected(show.runtime) || show.runtime === 'apple') {
    // Built-in actions run offline, so they need neither a model nor consent —
    // but they do need a folder, and free-form instructions need a model.
    if (custom) return 'Pick a built-in action, or set up an AI provider.'
    if (!show.folder) return 'Choose a folder for built-in actions.'
    return null
  }
  if (custom && !text) return 'Describe what to do, or pick a built-in action.'
  return null
}

/** The `start` event for the backend's `compose_request`. */
export function startEvent(petId: number, draft: Draft, parentId: number | null) {
  const ask = draft.mode === 'ask'
  return {
    type: 'start',
    petId,
    prompt: draft.prompt,
    preset: ask ? '' : ACTIONS[draft.action]?.preset ?? '',
    mode: draft.mode,
    // Asking is the consent: the question goes, with the pet's folder if it has one.
    allowCloud: ask ? true : draft.allowCloud,
    allowWeb: ask && draft.allowWeb,
    parentId,
  }
}

/** What the bubble shows about this pet's task. */
export interface TaskView {
  id: number
  status: string
  done: boolean
  /** The approval being asked for, if the task is waiting on one. */
  approval: { id: number; summary: ApprovalSummary; title: string } | null
  /** Completed file changes that can still be undone. */
  undoable: boolean
  progress: string
  error: string
  result: string
}

export function taskView(task: Task): TaskView {
  const done = terminal(task.status)
  const waiting = task.status === 'waiting' && !!task.approval
  return {
    id: task.id,
    status: task.status,
    done,
    approval: waiting && task.approval
      ? { id: task.approval.id, summary: task.approval.summary, title: task.approval.title }
      : null,
    undoable: done && task.operations.some(o => o.state === 'applied' || o.state === 'applying'),
    progress: task.progress ?? '',
    error: typeof task.error === 'string' ? task.error : '',
    result: task.result ?? '',
  }
}

export const isTask = (update: BubbleUpdate): update is Task =>
  typeof (update as Task).id === 'number' && typeof (update as Task).request === 'object'

/** Height of the tail, and half its base width. */
export const NOTCH = 16
const HALF = 14
const RADIUS = 20

/** The whole bubble outline, tail included, as one closed path in CSS pixels
 *  (y down): the border then carries across the tail's opening instead of
 *  cutting the bubble in two. The tail tip is at `tailX`, clamped clear of the
 *  rounded corners, on the bottom edge — or the top one with `tailUp`, for
 *  when the bubble had to go below its pet. */
export function bubblePath(w: number, h: number, tailX: number, tailUp: boolean): string {
  const r = RADIUS, k = r * 0.55
  const tip = Math.max(r + HALF + 4, Math.min(tailX, w - r - HALF - 4))
  const f = (n: number) => Math.round(n * 100) / 100
  const p = (...points: number[]) => points.map(f).join(' ')
  if (tailUp) {
    const t = NOTCH
    return [
      `M${p(tip - HALF, t)}`, `L${p(tip, 0)}`, `L${p(tip + HALF, t)}`, `L${p(w - r, t)}`,
      `C${p(w - k, t, w, t + k, w, t + r)}`, `L${p(w, h - r)}`,
      `C${p(w, h - k, w - k, h, w - r, h)}`, `L${p(r, h)}`,
      `C${p(k, h, 0, h - k, 0, h - r)}`, `L${p(0, t + r)}`,
      `C${p(0, t + k, k, t, r, t)}`, 'Z',
    ].join(' ')
  }
  const b = h - NOTCH
  return [
    `M${p(tip + HALF, b)}`, `L${p(tip, h)}`, `L${p(tip - HALF, b)}`, `L${p(r, b)}`,
    `C${p(k, b, 0, b - k, 0, b - r)}`, `L${p(0, r)}`,
    `C${p(0, k, k, 0, r, 0)}`, `L${p(w - r, 0)}`,
    `C${p(w - k, 0, w, k, w, r)}`, `L${p(w, b - r)}`,
    `C${p(w, b - k, w - k, b, w - r, b)}`, 'Z',
  ].join(' ')
}

/** The end of a long path, which is the part that tells folders apart. */
export function shortPath(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean)
  return parts.length > 2 ? `…${path.includes('\\') ? '\\' : '/'}${parts.slice(-2).join(path.includes('\\') ? '\\' : '/')}` : path
}

/** The first-run tour, one card per thing to try. The pet says it in its own voice. */
export const TOUR = [
  { title: 'Hi, I live on your windows', text: 'Drag me anywhere and I will hang on where you let go. I climb and perch on window edges by myself.' },
  { title: 'Click me for a chat', text: 'Right-click me to open this bubble. Actions like sorting or finding files, and questions, start here.' },
  { title: 'Hand me a folder', text: 'Choose a folder here, or drag me onto one in your file manager where that is supported. I only work in the folder you give me.' },
] as const

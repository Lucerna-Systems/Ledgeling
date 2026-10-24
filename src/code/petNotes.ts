import { computed, ref, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { codeState, root } from './state'
import { live } from './live'
import { terms } from '../terminal/terminals'
import type { Note, Tone } from './types'

/** The worktree on screen's notes: what its pet noticed, oldest first. */
export const notes = shallowRef<Note[]>([])
/** The newest note the user has had the side chat open for. */
const seen = ref(0)
export const unreadNotes = computed(() => notes.value.some(n => n.id > seen.value))
export function markSeen() { seen.value = Math.max(seen.value, notes.value.at(-1)?.id ?? 0) }

async function fetchNotes(path: string | null) {
  if (!path) { notes.value = []; return }
  try {
    const list = await invoke<Note[]>('code_notes', { worktree: path, after: 0 })
    if (root.value === path) notes.value = list
  } catch { /* the next change tries again */ }
}

/** Something the window noticed itself, said by the worktree's pet. */
export function say(worktree: string, text: string, tone: Tone = 'info') {
  return invoke('code_note', { worktree, text, tone }).then(() => fetchNotes(root.value)).catch(() => {})
}

/** Ports and desktop apps are only seen while the window polls for them, so
 *  the window reports those; Rust reports terminals starting, asking and ending. */
function narrate() {
  let known: Set<string> | null = null
  watch(live, now => {
    const keys = new Map<string, { worktree: string; text: string }>()
    for (const port of now.ports) {
      const t = port.session === null ? undefined : terms.get(port.session)
      if (t?.worktree) keys.set(`p${port.session}:${port.port}`, { worktree: t.worktree, text: `${t.title} is serving localhost:${port.port}.` })
    }
    for (const app of now.apps) {
      const t = terms.get(app.session)
      if (t?.worktree) keys.set(`a${app.pid}`, { worktree: t.worktree, text: `${app.name || 'The app'} is open, from ${t.title}.` })
    }
    // What was already there when the window opened is not news.
    if (known) for (const [key, note] of keys) if (!known.has(key)) void say(note.worktree, note.text, 'good')
    known = new Set(keys.keys())
  })
}

let started = false
export function startNotes() {
  if (started) return
  started = true
  watch([root, () => codeState.value.notes], ([path]) => void fetchNotes(path), { immediate: true })
  narrate()
}

import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useBackend } from './backend'
import type { AgentTool, SessionView } from './types'

/** Terminal mode: the terminals of the pet that opened this window. */
export interface Context { petId: number; petName: string; folder: string }
interface TerminalInfo { context: Context | null; sessions: SessionView[]; agents: AgentTool[]; layouts: Record<string, unknown> }

export const info = ref<TerminalInfo>({ context: null, sessions: [], agents: [], layouts: {} })
export const problem = ref('')
/** The pet whose terminals this view shows. The terminal window leaves it unset and follows the pet that opened it; the bubble names its pet. */
export const pet = ref<number | null>(null)
export const claude = computed(() => info.value.agents.some(a => a.id === 'claude'))
export const scope = computed(() => info.value.context?.folder ?? null)

export async function refresh() {
  try { info.value = await invoke<TerminalInfo>('terminal_state', { pet: pet.value }) } catch (e) { problem.value = String(e) }
}

export const statusOf = (id: number) => info.value.sessions.find(s => s.id === id)?.agent ?? null

useBackend({
  scope,
  spawn: (options, size, channel) => invoke<number>('terminal_spawn', { folder: options.worktree, kind: options.kind, command: options.command ?? null, title: options.title, ...size, channel }),
  io: (id, action, data, cols, rows) => invoke('terminal_io', { id, action, data, cols, rows }),
  attach: (id, channel) => invoke('terminal_attach', { id, channel }),
  sessions: () => info.value.sessions,
  layouts: async () => info.value.layouts,
  saveLayout: (folder, layout) => invoke('terminal_layout_save', { folder, layout }),
  changed: () => void refresh(),
  openUrl: url => { void invoke('terminal_open_url', { url }).catch(() => {}) },
  fail: message => { problem.value = message },
})

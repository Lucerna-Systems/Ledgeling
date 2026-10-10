import { invoke } from '@tauri-apps/api/core'
import { useBackend } from '../terminal/backend'
import { codeState, problem, refreshState, root } from './state'

/** Coding mode's terminals live in its own service in Rust, scoped to the
 *  worktrees the user added; installers and run commands have entry points of their own. */
useBackend({
  scope: root,
  spawn(options, size, channel) {
    if (options.tool) return invoke<number>('code_install', { tool: options.tool, ...size, channel })
    if (options.runIndex !== undefined) return invoke<number>('code_run_start', { worktree: options.worktree, index: options.runIndex, ...size, channel })
    return invoke<number>('code_pty_spawn', { worktree: options.worktree, kind: options.kind, command: options.command ?? null, title: options.title, ...size, channel })
  },
  io: (id, action, data, cols, rows) => invoke('code_pty', { id, action, data, cols, rows }),
  attach: (id, channel) => invoke('code_pty_attach', { id, channel }),
  sessions: () => codeState.value.sessions,
  layouts: () => invoke<Record<string, unknown>>('code_layouts'),
  saveLayout: (worktree, layout) => invoke('code_layout_save', { worktree, layout }),
  changed: () => void refreshState(),
  openUrl: url => { void invoke('code_open_url', { url }).catch(() => {}) },
  fail: message => { problem.value = message },
})

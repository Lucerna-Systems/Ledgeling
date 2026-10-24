import type { Channel } from '@tauri-apps/api/core'
import type { Ref } from 'vue'
import type { Layout } from './layout'
import type { PtyOutput, SessionKind, SessionView } from './types'

export interface SpawnOptions { worktree: string; kind: SessionKind; command?: string | null; title: string; runIndex?: number; tool?: string }

/** What the terminal grid needs from whichever window hosts it. Coding mode
 *  and terminal mode each have their own commands in Rust; each window says
 *  which, once, at start-up. */
export interface TerminalBackend {
  /** The folder whose terminals are on screen. */
  scope: Readonly<Ref<string | null>>
  spawn(options: SpawnOptions, size: { cols: number; rows: number }, channel: Channel<PtyOutput>): Promise<number>
  io(id: number, action: 'write' | 'resize' | 'kill' | 'rename', data: string, cols: number, rows: number): Promise<unknown>
  attach(id: number, channel: Channel<PtyOutput>): Promise<unknown>
  /** Terminals still running in Rust from before this window was opened. */
  sessions(): SessionView[]
  layouts(): Promise<Record<string, unknown>>
  saveLayout(scope: string, layout: Layout | null): Promise<unknown>
  /** A link clicked in a terminal. */
  openUrl(url: string): void
  /** Terminals were added or closed: the rest of the window should re-read. */
  changed(): void
  fail(message: string): void
}

let current: TerminalBackend | null = null
export function useBackend(backend: TerminalBackend) { current = backend }
export function backend(): TerminalBackend {
  if (!current) throw new Error('No terminal backend was set for this window.')
  return current
}

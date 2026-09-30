import { markRaw, reactive, watch } from 'vue'
import { Channel, invoke } from '@tauri-apps/api/core'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import '@xterm/xterm/css/xterm.css'
import { palette, terminalTheme, trackPalette } from './theme'
import { splitShortcut } from './shortcuts'
import { codeState, problem } from './state'
import type { PtyOutput, SessionKind } from './types'

/** One terminal per backend session. The xterm instance and its element
 *  outlive any view showing them, so switching tabs never loses scrollback. */
export interface Term {
  id: number
  kind: SessionKind
  worktree: string
  title: string
  alive: boolean
  exitCode: number | null
  term: Terminal
  fit: FitAddon
  element: HTMLDivElement
  /** xterm measures fonts when opened, so it opens on first display, not before. */
  opened: boolean
}

export const terms = reactive(new Map<number, Term>())

function create(): { term: Terminal; fit: FitAddon; element: HTMLDivElement } {
  trackPalette()
  const term = new Terminal({
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
    fontSize: 12.5,
    lineHeight: 1.2,
    cursorBlink: true,
    allowProposedApi: true,
    scrollback: 5000,
    theme: palette.value ? terminalTheme(palette.value) : undefined,
  })
  const fit = new FitAddon()
  term.loadAddon(fit)
  term.loadAddon(new WebLinksAddon((_, url) => { void invoke('code_open_url', { url }).catch(() => {}) }))
  // Split keys belong to the grid, not the program in the terminal.
  term.attachCustomKeyEventHandler(event => !splitShortcut(event))
  const element = document.createElement('div')
  element.className = 'term-host'
  // Vue must not proxy xterm's internals.
  return { term: markRaw(term), fit: markRaw(fit), element: markRaw(element) }
}

watch(palette, p => { if (p) for (const t of terms.values()) t.term.options.theme = terminalTheme(p) })

function connect(entry: Term): Channel<PtyOutput> {
  const channel = new Channel<PtyOutput>()
  channel.onmessage = message => {
    if (message.type === 'data') entry.term.write(message.data)
    else {
      entry.alive = false
      entry.exitCode = message.code
      entry.term.write(`\r\n\x1b[2m— finished${message.code ? ` with code ${message.code}` : ''} —\x1b[0m\r\n`)
    }
  }
  return channel
}

function wire(entry: Term) {
  entry.term.onData(data => { void invoke('code_pty', { id: entry.id, action: 'write', data, cols: 0, rows: 0 }).catch(() => {}) })
  entry.term.onResize(({ cols, rows }) => { void invoke('code_pty', { id: entry.id, action: 'resize', data: '', cols, rows }).catch(() => {}) })
}

export interface SpawnOptions { worktree: string; kind: SessionKind; command?: string | null; title: string; runIndex?: number; tool?: string }

/** Start a program in a new terminal. Run commands and installers have their own backend entry points. */
export async function spawn(options: SpawnOptions): Promise<Term | null> {
  const { term, fit, element } = create()
  const entry = reactive<Term>({ id: 0, kind: options.kind, worktree: options.worktree, title: options.title, alive: true, exitCode: null, term, fit, element, opened: false }) as Term
  const channel = connect(entry)
  const size = { cols: term.cols || 100, rows: term.rows || 30 }
  try {
    entry.id = options.tool
      ? await invoke<number>('code_install', { tool: options.tool, ...size, channel })
      : options.runIndex !== undefined
        ? await invoke<number>('code_run_start', { worktree: options.worktree, index: options.runIndex, ...size, channel })
        : await invoke<number>('code_pty_spawn', { worktree: options.worktree, kind: options.kind, command: options.command ?? null, title: options.title, ...size, channel })
  } catch (e) {
    problem.value = String(e)
    term.dispose()
    return null
  }
  wire(entry)
  terms.set(entry.id, entry)
  return entry
}

/** Sessions still running in Rust from before the window was last closed. */
export async function adopt() {
  for (const session of codeState.value.sessions) {
    if (terms.has(session.id) || !session.alive) continue
    const { term, fit, element } = create()
    const entry = reactive<Term>({ id: session.id, kind: session.kind, worktree: session.worktree, title: session.title, alive: true, exitCode: null, term, fit, element, opened: false }) as Term
    terms.set(session.id, entry)
    wire(entry)
    try { await invoke('code_pty_attach', { id: session.id, channel: connect(entry) }) } catch { terms.delete(session.id) }
  }
}

export async function close(id: number) {
  const entry = terms.get(id)
  terms.delete(id)
  entry?.term.dispose()
  await invoke('code_pty', { id, action: 'kill', data: '', cols: 0, rows: 0 }).catch(() => {})
}

export function write(id: number, data: string) {
  return invoke('code_pty', { id, action: 'write', data, cols: 0, rows: 0 })
}

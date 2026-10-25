import { markRaw, reactive, watch } from 'vue'
import { Channel } from '@tauri-apps/api/core'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import '@xterm/xterm/css/xterm.css'
import { palette, terminalTheme, trackPalette } from './theme'
import { splitShortcut } from './shortcuts'
import { backend, type SpawnOptions } from './backend'
import type { PtyOutput, SessionKind, SessionView } from './types'

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
  term.loadAddon(new WebLinksAddon((_, url) => { backend().openUrl(url) }))
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
  entry.term.onData(data => { void backend().io(entry.id, 'write', data, 0, 0).catch(() => {}) })
  entry.term.onResize(({ cols, rows }) => { void backend().io(entry.id, 'resize', '', cols, rows).catch(() => {}) })
}

/** Start a program in a new terminal. */
export async function spawn(options: SpawnOptions): Promise<Term | null> {
  const { term, fit, element } = create()
  const entry = reactive<Term>({ id: 0, kind: options.kind, worktree: options.worktree, title: options.title, alive: true, exitCode: null, term, fit, element, opened: false }) as Term
  const channel = connect(entry)
  const size = { cols: term.cols || 100, rows: term.rows || 30 }
  try {
    entry.id = await backend().spawn(options, size, channel)
  } catch (e) {
    backend().fail(String(e))
    term.dispose()
    return null
  }
  wire(entry)
  terms.set(entry.id, entry)
  return entry
}

/** Sessions still running in Rust from before the window was last closed.
 *  `only` limits it to the ones this view shows. */
export async function adopt(only: (session: SessionView) => boolean = () => true) {
  for (const session of backend().sessions()) {
    if (terms.has(session.id) || !session.alive || !only(session)) continue
    const { term, fit, element } = create()
    const entry = reactive<Term>({ id: session.id, kind: session.kind, worktree: session.worktree, title: session.title, alive: true, exitCode: null, term, fit, element, opened: false }) as Term
    terms.set(session.id, entry)
    wire(entry)
    try { await backend().attach(session.id, connect(entry)) } catch { terms.delete(session.id) }
  }
}

export async function close(id: number) {
  const entry = terms.get(id)
  terms.delete(id)
  entry?.term.dispose()
  await backend().io(id, 'kill', '', 0, 0).catch(() => {})
}

export function write(id: number, data: string) {
  return backend().io(id, 'write', data, 0, 0)
}

/** Give a terminal's tab a new name. Kept in Rust too, so it survives the view closing. */
export function rename(id: number, title: string) {
  const entry = terms.get(id)
  const clean = title.replace(/\s+/g, ' ').trim().slice(0, 40)
  if (!entry || !clean || clean === entry.title) return
  entry.title = clean
  void backend().io(id, 'rename', clean, 0, 0).catch(() => {})
}

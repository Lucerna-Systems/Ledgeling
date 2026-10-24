import { codeState, status } from './state'
import { live } from './live'
import { terms } from '../terminal/terminals'
import type { Note } from './types'

export interface DigestTerminal { title: string; state: string; tail: string[] }
export interface Digest { terminals: DigestTerminal[]; serving: string[]; changes: string[]; branch: string }

/** The last non-empty lines a terminal shows, as plain text. */
function tail(id: number, count: number): string[] {
  const buffer = terms.get(id)?.term.buffer.active
  if (!buffer) return []
  const out: string[] = []
  for (let y = buffer.baseY + buffer.cursorY; y >= 0 && out.length < count; y--) {
    const text = buffer.getLine(y)?.translateToString(true).trimEnd()
    if (text) out.unshift(text)
  }
  return out
}

/** What is happening in a worktree right now, read on this machine without
 *  any AI: each terminal and its last lines, what is being served, and git. */
export function digest(worktree: string): Digest {
  const sessions = new Map(codeState.value.sessions.map(s => [s.id, s]))
  const terminals = [...terms.values()].filter(t => t.worktree === worktree).map(t => {
    const s = sessions.get(t.id)
    const state = !t.alive ? (t.exitCode ? `ended with code ${t.exitCode}` : 'ended')
      : s?.attention === 'needs_user' ? 'waiting for you'
        : s?.attention === 'busy' ? 'working'
          : t.kind === 'agent' ? 'idle' : 'running'
    return { title: t.title, state, tail: tail(t.id, 6) }
  })
  const ids = new Set([...terms.values()].filter(t => t.worktree === worktree).map(t => t.id))
  const serving = [
    ...live.value.apps.filter(a => ids.has(a.session)).map(a => `${a.name} (desktop app, from ${terms.get(a.session)?.title})`),
    ...live.value.ports.filter(p => p.session !== null && ids.has(p.session)).map(p => `http://localhost:${p.port} (from ${terms.get(p.session!)?.title})`),
  ]
  const git = status.value
  const changes = (git?.files ?? []).slice(0, 12).map(f => `${f.untracked ? 'new' : 'changed'} ${f.path}${f.added != null ? ` (+${f.added} −${f.removed ?? 0})` : ''}`)
  if ((git?.files.length ?? 0) > 12) changes.push(`and ${git!.files.length - 12} more`)
  const branch = git ? `${git.branch ?? 'detached'}${git.ahead ? `, ${git.ahead} commit${git.ahead === 1 ? '' : 's'} ahead` : ''}${git.behind ? `, ${git.behind} behind` : ''}` : ''
  return { terminals, serving, changes, branch }
}

/** The digest as text, for the chat model to answer from. */
export function digestText(d: Digest, recent: Note[]): string {
  const parts = [`Branch: ${d.branch || 'unknown'}`]
  parts.push(d.terminals.length
    ? 'Terminals:\n' + d.terminals.map(t => `- ${t.title}: ${t.state}${t.tail.length ? `\n  last lines:\n${t.tail.map(l => `    ${l}`).join('\n')}` : ''}`).join('\n')
    : 'Terminals: none')
  parts.push(`Serving: ${d.serving.length ? d.serving.join('; ') : 'nothing'}`)
  parts.push(d.changes.length ? 'Uncommitted changes:\n' + d.changes.map(c => `- ${c}`).join('\n') : 'Uncommitted changes: none')
  if (recent.length) parts.push('Recent events:\n' + recent.slice(-6).map(n => `- ${n.text}`).join('\n'))
  return parts.join('\n\n')
}

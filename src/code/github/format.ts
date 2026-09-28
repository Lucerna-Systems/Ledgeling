import type { Check, Kind, PrSummary } from './types'

export function ago(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return ''
  const seconds = Math.max(0, (now - new Date(iso).getTime()) / 1000)
  if (seconds < 60) return 'just now'
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
  if (seconds < 604800) return `${Math.floor(seconds / 86400)}d ago`
  if (seconds < 2592000) return `${Math.floor(seconds / 604800)}w ago`
  return new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: seconds > 31536000 ? 'numeric' : undefined })
}

/** Avatars are drawn, never fetched: the window loads nothing from GitHub. */
export function hue(login: string) {
  let h = 0
  for (const c of login) h = (h * 31 + c.charCodeAt(0)) % 360
  return h
}
export const initials = (login: string) => login.replace(/\[bot\]$/, '').replace(/[^a-zA-Z0-9]/g, '').slice(0, 2).toUpperCase() || '?'

export const labelStyle = (color: string) => ({ '--label': `#${/^[0-9a-f]{6}$/i.test(color) ? color : '8b949e'}` })

/** Projects v2 option colours, as GitHub names them. */
const PROJECT_COLORS: Record<string, string> = {
  GRAY: '#8b949e', BLUE: '#4493f8', GREEN: '#3fb950', YELLOW: '#d29922',
  ORANGE: '#db6d28', RED: '#f85149', PINK: '#db61a2', PURPLE: '#a371f7',
}
export const projectColor = (name: string) => PROJECT_COLORS[name] ?? PROJECT_COLORS.GRAY

export type CheckState = 'pass' | 'fail' | 'pending' | 'skipped'
export function checkState(check: Check): CheckState {
  if (check.__typename === 'StatusContext') {
    const s = (check.state ?? '').toUpperCase()
    return s === 'SUCCESS' ? 'pass' : s === 'PENDING' || s === 'EXPECTED' ? 'pending' : 'fail'
  }
  if ((check.status ?? '').toUpperCase() !== 'COMPLETED') return 'pending'
  const c = (check.conclusion ?? '').toUpperCase()
  if (c === 'SUCCESS' || c === 'NEUTRAL') return 'pass'
  if (c === 'SKIPPED') return 'skipped'
  return 'fail'
}
export const checkName = (check: Check) => check.name ?? check.context ?? 'Check'
export const checkUrl = (check: Check) => check.detailsUrl ?? check.targetUrl ?? null

export interface Rollup { state: 'pass' | 'fail' | 'pending' | 'none'; passed: number; failed: number; pending: number; total: number }
export function rollup(checks: Check[] | null | undefined): Rollup {
  const states = (checks ?? []).map(checkState).filter(s => s !== 'skipped')
  const passed = states.filter(s => s === 'pass').length
  const failed = states.filter(s => s === 'fail').length
  const pending = states.filter(s => s === 'pending').length
  const state = !states.length ? 'none' : failed ? 'fail' : pending ? 'pending' : 'pass'
  return { state, passed, failed, pending, total: states.length }
}
export function rollupText(r: Rollup) {
  if (r.state === 'none') return 'No checks'
  if (r.state === 'fail') return `${r.failed} of ${r.total} checks failing`
  if (r.state === 'pending') return `${r.pending} of ${r.total} checks running`
  return r.total === 1 ? 'Check passed' : `All ${r.total} checks passed`
}

export type PrState = 'open' | 'draft' | 'merged' | 'closed'
export const prState = (pr: Pick<PrSummary, 'state' | 'isDraft'>): PrState =>
  pr.state === 'MERGED' ? 'merged' : pr.state === 'CLOSED' ? 'closed' : pr.isDraft ? 'draft' : 'open'

export interface DiffLine { kind: 'add' | 'del' | 'ctx' | 'note'; text: string; old: number | null; new: number | null }
export interface DiffHunk { header: string; lines: DiffLine[] }
export interface DiffFile { path: string; oldPath: string | null; status: 'added' | 'deleted' | 'renamed' | 'modified'; added: number; removed: number; binary: boolean; hunks: DiffHunk[] }

/** A unified diff (`gh pr diff`) split into files and hunks with line numbers. */
export function parseDiff(text: string): DiffFile[] {
  const files: DiffFile[] = []
  let file: DiffFile | null = null
  let hunk: DiffHunk | null = null
  let oldLine = 0
  let newLine = 0
  for (const line of text.split('\n')) {
    if (line.startsWith('diff --git ')) {
      const match = /^diff --git a\/(.*) b\/(.*)$/.exec(line)
      file = { path: match?.[2] ?? line.slice(11), oldPath: null, status: 'modified', added: 0, removed: 0, binary: false, hunks: [] }
      files.push(file)
      hunk = null
      continue
    }
    if (!file) continue
    if (!hunk) {
      if (line.startsWith('new file mode')) file.status = 'added'
      else if (line.startsWith('deleted file mode')) file.status = 'deleted'
      else if (line.startsWith('rename from ')) { file.status = 'renamed'; file.oldPath = line.slice(12) }
      else if (line.startsWith('Binary files')) file.binary = true
    }
    const header = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/.exec(line)
    if (header) {
      oldLine = Number(header[1])
      newLine = Number(header[2])
      hunk = { header: header[3].trim(), lines: [] }
      file.hunks.push(hunk)
      continue
    }
    if (!hunk) continue
    if (line.startsWith('+')) { hunk.lines.push({ kind: 'add', text: line.slice(1), old: null, new: newLine++ }); file.added++ }
    else if (line.startsWith('-')) { hunk.lines.push({ kind: 'del', text: line.slice(1), old: oldLine++, new: null }); file.removed++ }
    else if (line.startsWith(' ')) hunk.lines.push({ kind: 'ctx', text: line.slice(1), old: oldLine++, new: newLine++ })
    else if (line.startsWith('\\')) hunk.lines.push({ kind: 'note', text: line.slice(2), old: null, new: null })
  }
  return files
}

/** `#12` in prose becomes a link to this repository, outside code. */
export function linkRefs(markdown: string, repoUrl: string): string {
  return markdown.split(/(```[\s\S]*?```|`[^`\n]*`)/g).map((part, i) =>
    i % 2 ? part : part.replace(/(^|[\s(])#(\d+)\b/g, (_, lead: string, n: string) => `${lead}[#${n}](${repoUrl}/issues/${n})`),
  ).join('')
}

/** A link to an issue or pull request in this repository, which opens here. */
export function localTarget(url: string, repoUrl: string): { kind: Kind; number: number } | null {
  const base = repoUrl.replace(/\/$/, '').toLowerCase()
  const match = /^(.*)\/(issues|pull)\/(\d+)(?:[/?#].*)?$/i.exec(url)
  if (!match || match[1].toLowerCase() !== base) return null
  return { kind: match[2].toLowerCase() === 'pull' ? 'pr' : 'issue', number: Number(match[3]) }
}

/** A branch name for starting work on an issue: `issue-12-fix-the-thing`. */
export function issueBranch(number: number, title: string) {
  const slug = title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').split('-').slice(0, 6).join('-')
  return `issue-${number}${slug ? `-${slug}` : ''}`
}

/** One shell argument, for a command typed into the user's own terminal. */
export function shellQuote(text: string, windows = navigator.userAgent.includes('Windows')) {
  return windows ? `"${text.replace(/"/g, '\\"')}"` : `'${text.replace(/'/g, `'\\''`)}'`
}

/** The Actions run behind a check's link, for re-running its failed jobs. */
export function runId(url: string | null | undefined): number | null {
  const match = /\/actions\/runs\/(\d+)/.exec(url ?? '')
  return match ? Number(match[1]) : null
}

export type Tab = 'trees' | 'files' | 'editor' | 'changes' | 'github' | 'worktrees' | 'run' | 'terminal' | 'chat'
/** The two views that fill the main area of the wide layout. */
export type MainView = 'terminal' | 'editor' | 'changes' | 'github'

export interface Worktree { path: string; branch: string | null; head: string; main: boolean; locked: boolean; prunable: boolean }
export interface WorktreeView extends Worktree { name: string; petId: number | null }
export interface ProjectView { path: string; name: string; worktrees: WorktreeView[]; problem: string | null }
export type SessionKind = 'shell' | 'agent' | 'run'
export interface SessionView { id: number; kind: SessionKind; worktree: string; title: string; pid: number | null; alive: boolean; attention: 'busy' | 'needs_user' | null }
export interface CodeState { projects: ProjectView[]; selected: string | null; pinned: boolean; focus: boolean; sessions: SessionView[]; notes: number }
export type Tone = 'info' | 'ask' | 'good' | 'bad'
export interface Note { id: number; at: number; worktree: string; text: string; tone: Tone }

export interface FileChange { path: string; original: string | null; staged: string; unstaged: string; untracked: boolean; conflicted: boolean; added: number | null; removed: number | null }
export interface GitStatus { branch: string | null; upstream: string | null; ahead: number; behind: number; files: FileChange[] }
export interface Changes { paths: string[]; git: boolean }

export interface Entry { name: string; path: string; dir: boolean; ignored: boolean }
export interface FileText { text: string; modified: number; binary: boolean; tooLarge: boolean }

export type PtyOutput = { type: 'data'; data: string } | { type: 'exit'; code: number | null }

export interface RunConfig { name: string; command: string; cwd: string; port: number | null; url: string | null; env: Record<string, string> }
export interface Diagnostic { tier: string; file: string; path: string; problem: string; expected: string }
export interface Suggestion { config: RunConfig; reason: string }
export interface RunSetup { configs: RunConfig[]; fromFile: boolean; problems: Diagnostic[]; suggestions: Suggestion[] }
export interface PortView { port: number; pid: number; process: string; exposed: boolean; session: number | null }
export interface AppView { session: number; pid: number; name: string }
export interface Live { ports: PortView[]; apps: AppView[] }
export interface Built { path: string; name: string; modified: number }
export interface NativeApp { framework: string; dev: string; build: string; built: Built | null }

export interface Tool { id: string; name: string; purpose: string; version: string | null; installer: string | null; homepage: string }

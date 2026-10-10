export type SessionKind = 'shell' | 'agent' | 'run'
/** What an agent that reports its own events (Claude Code's hooks) says it is doing. */
export interface AgentStatus { state: 'idle' | 'working' | 'waiting' | 'done'; detail: string | null }
export interface SessionView { id: number; kind: SessionKind; worktree: string; title: string; pid: number | null; alive: boolean; attention: 'busy' | 'needs_user' | null; agent: AgentStatus | null }
export type PtyOutput = { type: 'data'; data: string } | { type: 'exit'; code: number | null }
/** A coding agent installed on the user's PATH, started by running `command` in a terminal. */
export interface AgentTool { id: string; name: string; command: string }

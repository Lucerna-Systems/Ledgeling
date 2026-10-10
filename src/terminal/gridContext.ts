import type { Component, ComputedRef, InjectionKey } from 'vue'
import type { Direction } from './layout'
import type { AgentStatus } from './types'

/** What a terminal grid hands down to its panes and dividers. */
export interface GridContext {
  worktree: ComputedRef<string>
  active: ComputedRef<boolean>
  focused: ComputedRef<number | null>
  focus(id: number): void
  split(id: number, dir: Direction): void
  close(id: number): void
  /** The layout was resized in place; keep it. */
  changed(): void
  /** What the agent in this terminal says it is doing, if it reports. */
  status(id: number): AgentStatus | null
  /** The host's own buttons for a pane, such as coding mode's "open what this serves". */
  tools: { component: Component; props(id: number): Record<string, unknown> } | null
}
export const gridKey: InjectionKey<GridContext> = Symbol('grid')

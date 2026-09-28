import type { ComputedRef, InjectionKey } from 'vue'
import type { Direction } from './layout'

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
}
export const gridKey: InjectionKey<GridContext> = Symbol('grid')

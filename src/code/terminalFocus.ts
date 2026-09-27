import { ref } from 'vue'
import type { SessionKind } from './types'

/** A terminal another tab wants started, picked up by the Terminal tab when it
 *  opens. Kept apart from terminals.ts so asking does not load xterm. */
export interface PendingTerminal { worktree: string; title: string; kind?: SessionKind; command?: string; tool?: string }
export const pendingTerminal = ref<PendingTerminal | null>(null)

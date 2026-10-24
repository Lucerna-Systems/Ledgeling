import { invoke } from '@tauri-apps/api/core'
import { watch, type WatchSource } from 'vue'

/**
 * UI state that exists only in the companion webview. The companion window is
 * destroyed when it has been hidden for a while, so this is written to Rust
 * (`ui_state.json`) as it changes and read back by `companion_ready`.
 */
export interface UiState {
  page: string
  /** The unsent assistant request. */
  prompt: string
  /** The Pet Studio design in progress, in `serializeDraft` form. */
  studio: unknown
  /** True while the page holds input that is not saved here; Rust then never destroys the window. */
  volatile: boolean
}

export interface Ready {
  ui: Partial<UiState> | null
  pending: { event: string; payload: unknown }[]
}

export const companionReady = () => invoke<Ready>('companion_ready')

/** Save the snapshot shortly after any source changes. Returns a function that flushes and stops. */
export function startPersist(sources: WatchSource[], snapshot: () => UiState, delay = 400): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined
  const save = () => {
    timer = undefined
    invoke('companion_save_ui', { json: JSON.stringify(snapshot()) }).catch(() => {})
  }
  const stop = watch(sources, () => { if (timer === undefined) timer = setTimeout(save, delay) }, { deep: true })
  return () => { stop(); if (timer !== undefined) { clearTimeout(timer); save() } }
}

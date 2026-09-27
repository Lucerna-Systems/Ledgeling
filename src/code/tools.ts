import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Tool } from './types'

/** Developer tools found on this machine; checked once per window and on demand. */
export const tools = ref<Tool[]>([])
let loading: Promise<void> | null = null
export function loadTools(force = false) {
  if (loading && !force) return loading
  loading = invoke<Tool[]>('code_tools').then(list => { tools.value = list }).catch(() => {})
  return loading
}
export const has = (id: string) => !!tools.value.find(t => t.id === id)?.version

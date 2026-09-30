import { computed, getCurrentScope, onScopeDispose, reactive, shallowRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { Live } from './types'

/** Ports and desktop apps the window's terminals have opened, shared by
 *  every view that shows them so the socket table is read once per tick. */
export const live = shallowRef<Live>({ ports: [], apps: [] })

const wants = reactive(new Map<symbol, boolean>())
const wanted = computed(() => [...wants.values()].some(Boolean))

export async function refreshLive() {
  try { live.value = await invoke<Live>('code_live') } catch { /* the next tick tries again */ }
}

let timer: ReturnType<typeof setInterval> | undefined
watch(wanted, on => {
  clearInterval(timer)
  if (on) { void refreshLive(); timer = setInterval(refreshLive, 2000) }
})

/** Poll only while `want` is true for at least one view. */
export function useLive(want: () => boolean) {
  const key = Symbol('live')
  watch(want, on => { wants.set(key, on) }, { immediate: true })
  if (getCurrentScope()) onScopeDispose(() => { wants.delete(key) })
}

export const portsOf = (session: number) => live.value.ports.filter(p => p.session === session)
export const appsOf = (session: number) => live.value.apps.filter(a => a.session === session)
export const urlFor = (port: number) => `http://localhost:${port}`

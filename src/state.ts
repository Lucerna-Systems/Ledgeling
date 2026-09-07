import { ref, computed } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { PackListing, Report, Settings, Snapshot } from './types'

/** One polled copy of the backend's truth, shared by every panel. */
export const native = isTauri()
export const snapshot = ref<Snapshot | null>(null)
export const error = ref('')
export const busy = ref(false)
export const packs = ref<PackListing[]>([])

let polling = false

export async function refresh() {
  if (!native || polling || document.hidden) return
  polling = true
  try {
    snapshot.value = await invoke<Snapshot>('get_snapshot')
  } catch (e) {
    error.value = String(e)
  } finally {
    polling = false
  }
}

export async function refreshPacks() {
  if (!native) return
  try {
    packs.value = await invoke<PackListing[]>('list_packs')
  } catch (e) {
    error.value = String(e)
  }
}

export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T | undefined> {
  if (!native) return undefined
  busy.value = true
  error.value = ''
  try {
    return await invoke<T>(command, args)
  } catch (e) {
    error.value = String(e)
    return undefined
  } finally {
    busy.value = false
    await refresh()
  }
}

export async function action(kind: string) {
  const next = await call<Snapshot>('pet_action', { kind })
  if (next) snapshot.value = next
}

export async function saveSettings(next: Settings) {
  const saved = await call<Snapshot>('set_settings', { settings: next })
  if (saved) snapshot.value = saved
}

export async function validate(path: string) {
  return (await call<Report>('validate_pack', { path })) ?? { diagnostics: [] }
}

export function start() {
  if (!native) return () => {}
  void refresh()
  void refreshPacks()
  const timer = setInterval(refresh, 1000)
  const unlisten = listen('packs:changed', () => { void refreshPacks(); void refresh() })
  return () => {
    clearInterval(timer)
    void unlisten.then(stop => stop())
  }
}

export const settings = computed(() => snapshot.value?.settings ?? null)
export const pets = computed(() => snapshot.value?.pets ?? [])
export const notes = computed(() => snapshot.value?.notes ?? [])

export function minutes(value: number) {
  if (value < 1) return 'just started'
  if (value < 60) return `${Math.round(value)} min`
  const hours = Math.floor(value / 60)
  return `${hours} h ${Math.round(value % 60)} min`
}

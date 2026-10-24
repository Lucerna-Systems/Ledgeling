import { ref, computed } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { PackListing, Report, Settings, Snapshot } from './types'

/** One polled copy of the backend's truth, shared by every panel. */
export const native = isTauri()
export const snapshot = ref<Snapshot | null>(null)
export const error = ref('')
/** The stable `code` of the last command error, when Rust sent one. */
export const errorCode = ref('')
export const busy = ref(false)
export const packs = ref<PackListing[]>([])

let polling = false
let revision = 0
let pending = 0

export async function refresh() {
  if (!native || polling || busy.value || document.hidden) return
  polling = true
  const requestedRevision = revision
  try {
    const next = await invoke<Snapshot>('get_snapshot')
    // A poll started before an action must not overwrite its result.
    if (requestedRevision === revision) snapshot.value = next
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

export async function call<T>(command: string, args: Record<string, unknown> = {}, refreshAfter = true): Promise<T | undefined> {
  if (!native) return undefined
  pending++
  revision++
  busy.value = true
  error.value = ''
  errorCode.value = ''
  try {
    return await invoke<T>(command, args)
  } catch (e) {
    // Rust sends plain text, or `{ code, message }` for install and network errors.
    if (e && typeof e === 'object' && 'message' in e) {
      error.value = String((e as { message: unknown }).message)
      if ('code' in e) errorCode.value = String((e as { code: unknown }).code)
    } else {
      error.value = String(e)
    }
    return undefined
  } finally {
    busy.value = --pending > 0
    if (refreshAfter) void refresh()
  }
}

export async function action(kind: string) {
  const next = await call<Snapshot>('pet_action', { kind }, false)
  if (next) snapshot.value = next
}

export async function saveSettings(next: Settings) {
  const saved = await call<Snapshot>('set_settings', { settings: next }, false)
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
  const onVisible = () => { if (!document.hidden) void refresh() }
  document.addEventListener('visibilitychange', onVisible)
  window.addEventListener('focus', onVisible)
  const unlisten = listen('packs:changed', () => { void refreshPacks(); void refresh() })
  return () => {
    clearInterval(timer)
    document.removeEventListener('visibilitychange', onVisible)
    window.removeEventListener('focus', onVisible)
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

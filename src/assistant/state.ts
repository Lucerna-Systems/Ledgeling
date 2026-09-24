import { ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Task, Connections, Connection } from './types'
export const tasks = ref<Task[]>([])
export const taskError = ref('')
export const assignedPet = ref<number | null>(null)
export const droppedFiles = ref<string[]>([])
export const connections = ref<Connections>({ active: { kind: 'none', model: '', endpoint: '', executable: '' }, searchEnabled: false, perPet: {}, appleTerms: { acceptedVersion: 0, acceptedAt: 0 } })
/** The connection a task for this pet actually runs on: its override, else the default. */
export function effectiveConnection(petId: number | null): Connection {
  const config = connections.value
  return (petId !== null && config.perPet?.[petId]) || config.active
}
export async function taskCall<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!isTauri()) throw new Error('Launch the desktop app to use tasks.')
  taskError.value = ''
  try { return await invoke<T>(command, args) } catch (e) { taskError.value = String(e); throw e }
}
export async function refreshTasks() {
  if (!isTauri()) return
  tasks.value = await taskCall<Task[]>('task_list')
}
export async function startTasks() {
  if (!isTauri()) return () => {}
  const stop = await listen<Task>('task:changed', ({ payload }) => {
    tasks.value = [payload, ...tasks.value.filter(t => t.id !== payload.id)].sort((a, b) => b.id - a.id)
  })
  // Model picks can also happen in the pet bubble; keep the dashboard state truthful.
  const stopConnections = await listen('connections:changed', async () => {
    connections.value = await taskCall<Connections>('task_connections')
  })
  await refreshTasks()
  connections.value = await taskCall<Connections>('task_connections')
  return () => { stop(); stopConnections() }
}

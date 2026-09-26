import { computed, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Task, TaskPet, Connections, Connection, ProviderDetection, ProviderKind } from './types'
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
/** A chat: a first request and every follow-up chained to it through `parentId`. */
export interface Conversation { id: number; title: string; pet: TaskPet | null; petId: number | null; turns: Task[]; waiting: boolean }

/** The conversation open in the Assistant, by its first task's id; null is a new chat. */
export const activeConversation = ref<number | null>(null)

/** The first task of the chat `task` belongs to. */
function rootOf(task: Task, byId: Map<number, Task>): Task {
  let root = task
  // Bounded so a corrupt parent chain can never loop.
  for (let hops = 0; root.request.parentId !== null && hops < 10_000; hops++) {
    const parent = byId.get(root.request.parentId)
    if (!parent) break
    root = parent
  }
  return root
}

export function conversationTitle(root: Task): string {
  if (root.title) return root.title
  const line = root.request.prompt.trim().split('\n')[0] ?? ''
  return line.length > 48 ? `${line.slice(0, 47).trimEnd()}…` : line || 'New chat'
}

/** Chats, most recently active first; each chat's turns run oldest first. */
export const conversations = computed<Conversation[]>(() => {
  const byId = new Map(tasks.value.map(task => [task.id, task]))
  const groups = new Map<number, Task[]>()
  for (const task of tasks.value) {
    const root = rootOf(task, byId).id
    groups.set(root, [...(groups.get(root) ?? []), task])
  }
  return [...groups.entries()]
    .map(([id, turns]) => {
      const root = byId.get(id)!
      turns.sort((a, b) => a.id - b.id)
      return { id, title: conversationTitle(root), pet: root.pet, petId: root.request.petId, turns, waiting: turns.some(t => t.status === 'waiting') }
    })
    .sort((a, b) => b.turns.at(-1)!.id - a.turns.at(-1)!.id)
})

export function conversationOf(taskId: number): number {
  return conversations.value.find(chat => chat.turns.some(t => t.id === taskId))?.id ?? taskId
}

export async function renameConversation(id: number, title: string) {
  const task = await taskCall<Task>('task_rename', { id, title })
  tasks.value = tasks.value.map(t => t.id === task.id ? task : t)
}
/** Fields a fresh connection of each kind starts with. */
export const connectionDefaults: Record<ProviderKind, Partial<Connection>> = {
  none: {}, ollama: { endpoint: 'http://127.0.0.1:11434' }, openai: {}, anthropic: {},
  compatible: { endpoint: 'https://your-provider.example/v1' }, codex: { executable: 'codex' }, claude: {},
  apple: { model: 'system' },
}
/** A connection of `kind`, reusing the endpoint or executable already configured for it. */
export function draftConnection(kind: ProviderKind, model: string): Connection {
  const config = connections.value
  const existing = [config.active, ...Object.values(config.perPet ?? {})].find(c => c.kind === kind)
  return {
    kind, model: model || (connectionDefaults[kind].model ?? ''),
    endpoint: existing?.endpoint || (connectionDefaults[kind].endpoint ?? ''),
    executable: existing?.executable || (connectionDefaults[kind].executable ?? ''),
  }
}
/** What this machine has installed and which keys are saved; loaded once, shared by every picker. */
export const detection = ref<ProviderDetection | null>(null)
export async function loadDetection() {
  try { detection.value = await taskCall<ProviderDetection>('task_detect_providers') } catch { /* shared error */ }
}

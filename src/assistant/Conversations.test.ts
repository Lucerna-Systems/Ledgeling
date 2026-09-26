// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import AssistantPanel from './AssistantPanel.vue'
import ConversationList from './ConversationList.vue'
import { activeConversation, assignedPet, connections, conversations, tasks } from './state'
import type { Task } from './types'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: api.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const turn = (id: number, prompt: string, parentId: number | null, over: Partial<Task> = {}): Task => ({
  id, status: 'completed', request: { prompt, petId: null, roots: [], attachments: [], mode: 'standard', preset: 'find', parentId, allowCloud: false, allowWeb: false },
  connection: { kind: 'none', model: '', endpoint: '', executable: '' }, createdAt: 0, progress: '', result: `answer ${id}`, error: null,
  approval: null, operations: [], sources: [], pet: { packId: 'pip', name: 'Pip the Budgie' }, title: '', ...over,
})

beforeEach(() => {
  api.invoke.mockReset().mockResolvedValue(null)
  activeConversation.value = null
  assignedPet.value = null
  connections.value = { active: { kind: 'none', model: '', endpoint: '', executable: '' }, searchEnabled: false, perPet: {}, appleTerms: { acceptedVersion: 0, acceptedAt: 0 } }
  tasks.value = [turn(4, 'Something else', null), turn(3, 'And the PDFs?', 2), turn(2, 'Find my invoices', null), turn(1, 'Older chat', null)]
})

it('groups follow-ups into chats, most recently active first', () => {
  expect(conversations.value.map(chat => [chat.id, chat.title, chat.turns.map(t => t.id)])).toEqual([
    [4, 'Something else', [4]], [2, 'Find my invoices', [2, 3]], [1, 'Older chat', [1]],
  ])
})

it('shows an open chat as a thread and continues it from its latest turn', async () => {
  activeConversation.value = 2
  connections.value = { ...connections.value, active: { kind: 'ollama', model: 'llama', endpoint: 'http://127.0.0.1:11434', executable: '' } }
  const wrapper = mount(AssistantPanel)
  expect(wrapper.findAll('.chat-prompt').map(p => p.text())).toEqual(['Find my invoices', 'And the PDFs?'])
  expect(wrapper.get('.chat-heading h1').text()).toBe('Find my invoices')
  api.invoke.mockImplementation(async (command: string) => command === 'task_start' ? turn(5, 'Only this year', 3) : null)
  await wrapper.get('#task-prompt').setValue('Only this year')
  await wrapper.get('#task-prompt').trigger('keydown', { key: 'Enter' })
  await flushPromises()
  const [command, args] = api.invoke.mock.calls.find(([name]) => name === 'task_start')!
  expect(command).toBe('task_start')
  expect((args as { request: { parentId: number } }).request.parentId).toBe(3)
  expect(activeConversation.value).toBe(2)
  wrapper.unmount()
})

it('renames a chat from the sidebar', async () => {
  const wrapper = mount(ConversationList, { props: { current: true } })
  await wrapper.get('[aria-label="Rename Find my invoices"]').trigger('click')
  await wrapper.get('[aria-label="Conversation name"]').setValue('Tax season')
  api.invoke.mockResolvedValueOnce(turn(2, 'Find my invoices', null, { title: 'Tax season' }))
  await wrapper.get('[aria-label="Conversation name"]').trigger('keydown', { key: 'Enter' })
  await flushPromises()
  expect(api.invoke).toHaveBeenCalledWith('task_rename', { id: 2, title: 'Tax season' })
  expect(wrapper.findAll('.chat-title').map(t => t.text())).toContain('Tax season')
  wrapper.unmount()
})

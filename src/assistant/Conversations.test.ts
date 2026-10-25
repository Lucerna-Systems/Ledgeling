// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import AssistantPanel from './AssistantPanel.vue'
import ConversationList from './ConversationList.vue'
import { activeConversation, assignedPet, connections, conversations, conversationsByPet, tasks } from './state'
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
  connections.value = { active: { kind: 'none', model: '', endpoint: '', executable: '' }, searchEnabled: false, ollamaThinking: false, perPet: {}, providers: {} }
  tasks.value = [turn(4, 'Something else', null), turn(3, 'And the PDFs?', 2), turn(2, 'Find my invoices', null), turn(1, 'Older chat', null)]
})

it('groups follow-ups into chats, most recently active first', () => {
  expect(conversations.value.map(chat => [chat.id, chat.title, chat.turns.map(t => t.id)])).toEqual([
    [4, 'Something else', [4]], [2, 'Find my invoices', [2, 3]], [1, 'Older chat', [1]],
  ])
})

it('shows an open chat as a thread and continues it from its latest turn', async () => {
  activeConversation.value = 2
  connections.value = { ...connections.value, active: { kind: 'ollama', model: 'llama', endpoint: 'http://127.0.0.1:11434', executable: '' },
    providers: { ollama: { model: 'llama', endpoint: 'http://127.0.0.1:11434', executable: '', termsVersion: 0, acceptedAt: 0 } } }
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

it('groups chats under the pet they were with, newest pet first', async () => {
  const glass = { packId: 'glass', name: 'Glass the Gecko' }
  tasks.value = [turn(6, 'Gecko chat', null, { pet: glass }), ...tasks.value, turn(0, 'Nobody in particular', null, { pet: null })]
  expect(conversationsByPet.value.map(group => [group.name, group.chats.map(chat => chat.id)])).toEqual([
    ['Glass the Gecko', [6]], ['Pip the Budgie', [4, 2, 1]], ['Any pet', [0]],
  ])
  const wrapper = mount(ConversationList, { props: { current: true } })
  expect(wrapper.findAll('.pet-chats-toggle .chat-title').map(t => t.text())).toEqual(['Glass the Gecko', 'Pip the Budgie', 'Any pet'])
  await wrapper.findAll('.pet-chats-toggle')[1].trigger('click')
  expect(wrapper.findAll('.pet-chats-list .chat-title').map(t => t.text())).toEqual(['Gecko chat', 'Nobody in particular'])
  // A pet's own "+" starts a new chat; the pet itself is out only in the desktop app.
  activeConversation.value = 6
  await wrapper.get('[aria-label="New chat with Pip the Budgie"]').trigger('click')
  expect(activeConversation.value).toBeNull()
  wrapper.unmount()
})

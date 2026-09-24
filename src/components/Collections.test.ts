// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import PacksPanel from '../panels/PacksPanel.vue'
import AssistantPanel from '../assistant/AssistantPanel.vue'
import { packs, snapshot } from '../state'
import { tasks, connections, assignedPet } from '../assistant/state'
import type { PackListing, Snapshot } from '../types'
import type { Task } from '../assistant/types'
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => false, invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))
beforeEach(() => {
  packs.value = []; snapshot.value = null; tasks.value = []; assignedPet.value = null
  connections.value = { active: { kind: 'none', model: '', endpoint: '', executable: '' }, searchEnabled: false, perPet: {}, appleTerms: { acceptedVersion: 0, acceptedAt: 0 } }
})

const pack = (name: string, kind: string): PackListing => ({
  summary: {
    id: name, name, class: kind, author: 'Perch', description: 'A desktop friend', version: '1', builtin: true, loadable: true,
    errors: 0, warnings: 0, notes: 1, surfaces: [], preview: null, tags: [], license: '', animations: ['idle', 'move'], directory: '',
    assistantClips: [
      { slot: 'busy', when: 'while this pet is working on a task', clip: 'idle', provided: false },
      { slot: 'attention', when: 'while its task is waiting for your answer', clip: 'attention', provided: true },
    ],
  },
  report: { diagnostics: [] },
})
const task = (id: number, status: string): Task => ({
  id, status, request: { prompt: id === 1 ? 'Organise folder' : 'Research Vue', petId: null, roots: [], attachments: [], mode: 'standard', preset: '', parentId: null, allowCloud: false, allowWeb: false },
  connection: { kind: 'none', model: '', endpoint: '', executable: '' }, createdAt: 0, progress: '', result: '', error: null, approval: null, operations: [], sources: [],
})

it('combines case-insensitive pack search with a class filter and can clear both', async () => {
  packs.value = [pack('Pip', 'flier'), pack('Glass', 'climber')]
  const wrapper = mount(PacksPanel)
  await wrapper.get('[aria-label="Search pet packs"]').setValue('  GLASS ')
  expect(wrapper.findAll('.pack-card')).toHaveLength(1)
  expect(wrapper.get('.pack-card h3').text()).toBe('Glass')
  await wrapper.get('[aria-label="Search pet packs filter"]').setValue('flier')
  expect(wrapper.text()).toContain('No matching packs')
  await wrapper.get('[aria-label="Clear search and filters"]').trigger('click')
  expect(wrapper.findAll('.pack-card')).toHaveLength(2)
  wrapper.unmount()
})

it('says which assistant clips a pack supplies and what an empty slot plays instead', async () => {
  packs.value = [pack('Pip', 'flier')]
  const wrapper = mount(PacksPanel)
  const text = wrapper.get('.pack-more').text()
  expect(text).toContain('While this pet is working on a task')
  expect(text).toContain('Plays idle — this pack has no busy animation')
  expect(text).toContain('Plays its own attention')
  wrapper.unmount()
})

it('filters history by status and request and gives a useful empty state', async () => {
  tasks.value = [task(1, 'completed'), task(2, 'waiting')]
  const wrapper = mount(AssistantPanel)
  await wrapper.get('[aria-label="Search tasks filter"]').setValue('waiting')
  expect(wrapper.findAll('.task-card')).toHaveLength(1)
  expect(wrapper.get('.task-card').text()).toContain('Research Vue')
  await wrapper.get('[aria-label="Search tasks"]').setValue('organise')
  expect(wrapper.text()).toContain('No matching tasks')
  await wrapper.get('[aria-label="Clear search and filters"]').trigger('click')
  expect(wrapper.findAll('.task-card')).toHaveLength(2)
  wrapper.unmount()
})

it('limits history rendering and reveals additional tasks on request', async () => {
  tasks.value = Array.from({ length: 12 }, (_, index) => task(index + 1, 'completed'))
  const wrapper = mount(AssistantPanel)
  expect(wrapper.findAll('.task-card')).toHaveLength(10)
  await wrapper.get('.load-more').trigger('click')
  expect(wrapper.findAll('.task-card')).toHaveLength(12)
  wrapper.unmount()
})

it('selects the supported agent mode automatically and keeps built-in actions local', async () => {
  connections.value.active.kind = 'codex'
  const wrapper = mount(AssistantPanel)
  expect(wrapper.get('.request-options summary').text()).toContain('Agent')
  await wrapper.findAll('.task-shortcuts button').find(button => button.text() === 'Organise files')!.trigger('click')
  expect(wrapper.get('.request-options summary').text()).toContain('Built-in action')
  expect(wrapper.text()).toContain('Built-in action · runs locally')
  wrapper.unmount()
})

it('a companion override drives the mode and badge, and can be cleared back to the default', async () => {
  connections.value.active = { kind: 'ollama', model: 'llama', endpoint: 'http://127.0.0.1:11434', executable: '' }
  connections.value.perPet = { 1: { kind: 'codex', model: 'gpt-5.2-codex', endpoint: '', executable: 'codex' } }
  snapshot.value = {
    settings: { paused: false, hidden: false, speedScale: 1 }, notes: [], packs: [], perchCount: 0,
    pets: [{ id: 1, packId: 'pip', name: 'Pip', class: 'flier', state: 'rest', energy: 1, mood: .7, surface: null }],
  } as unknown as Snapshot
  const wrapper = mount(AssistantPanel)
  expect(wrapper.get('.connection-badge').text()).toContain('ollama')
  await wrapper.get('.composer-top select').setValue('1')
  expect(wrapper.get('.connection-badge').text()).toContain('codex')
  expect(wrapper.get('.connection-badge').text()).toContain('custom model')
  expect(wrapper.get('.request-options summary').text()).toContain('Agent')
  expect(wrapper.get('.task-meta').text()).toContain('gpt-5.2-codex')
  await wrapper.get('.task-meta .text-link').trigger('click')
  expect(connections.value.perPet[1]).toBeUndefined()
  await Promise.resolve()
  expect(wrapper.get('.request-options summary').text()).toContain('Ask AI')
  wrapper.unmount()
})

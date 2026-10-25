// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import PacksPanel from '../panels/PacksPanel.vue'
import AssistantPanel from '../assistant/AssistantPanel.vue'
import HistoryPanel from '../assistant/HistoryPanel.vue'
import { packs, snapshot } from '../state'
import { tasks, connections, assignedPet } from '../assistant/state'
import type { PackListing, Snapshot } from '../types'
import type { Task } from '../assistant/types'
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => false, invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))
beforeEach(() => {
  packs.value = []; snapshot.value = null; tasks.value = []; assignedPet.value = null
  connections.value = { active: { kind: 'none', model: '', endpoint: '', executable: '' }, searchEnabled: false, ollamaThinking: false, perPet: {}, providers: {} }
})

const pack = (name: string, kind: string): PackListing => ({
  summary: {
    id: name, name, class: kind, author: 'Ledgeling', description: 'A desktop friend', version: '1', builtin: true, loadable: true,
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
  connection: { kind: 'none', model: '', endpoint: '', executable: '' }, createdAt: 0, progress: '', result: '', error: null, approval: null, operations: [], sources: [], pet: null, title: '',
})

it('searches pets case-insensitively and can clear the search', async () => {
  packs.value = [pack('Pip', 'flier'), pack('Glass', 'climber')]
  const wrapper = mount(PacksPanel)
  await wrapper.get('[aria-label="Search pets"]').setValue('  GLASS ')
  expect(wrapper.findAll('.pack-card')).toHaveLength(1)
  expect(wrapper.get('.pack-card h3').text()).toBe('Glass')
  await wrapper.get('[aria-label="Search pets"]').setValue('nobody')
  expect(wrapper.text()).toContain('No pets match')
  await wrapper.get('[aria-label="Clear search"]').trigger('click')
  expect(wrapper.findAll('.pack-card')).toHaveLength(2)
  wrapper.unmount()
})

it('filters pets by who made them and how they move', async () => {
  packs.value = [pack('Pip', 'flier'), pack('Glass', 'climber'), { ...pack('Moss', 'hopper'), summary: { ...pack('Moss', 'hopper').summary, author: 'Mira', builtin: false } }]
  const wrapper = mount(PacksPanel)
  await wrapper.get('[aria-label="Made by"]').setValue('Mira')
  expect(wrapper.findAll('.pack-card h3').map(h => h.text())).toEqual(['Moss'])
  await wrapper.get('[aria-label="Made by"]').setValue('any')
  await wrapper.get('[aria-label="Source"]').setValue('builtin')
  await wrapper.get('[aria-label="How it moves"]').setValue('climber')
  expect(wrapper.findAll('.pack-card h3').map(h => h.text())).toEqual(['Glass'])
  await wrapper.get('[aria-label="Clear search and filters"]').trigger('click')
  expect(wrapper.findAll('.pack-card')).toHaveLength(3)
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

it('filters history by status tab, kind and search, and can clear them all', async () => {
  tasks.value = [task(1, 'completed'), task(2, 'waiting')]
  const wrapper = mount(HistoryPanel)
  await wrapper.get('[aria-label="Status"]').findAll('button').find(button => button.text().startsWith('Needs you'))!.trigger('click')
  expect(wrapper.findAll('.task-card')).toHaveLength(1)
  expect(wrapper.get('.task-card').text()).toContain('Research Vue')
  await wrapper.get('[aria-label="Search history"]').setValue('organise')
  expect(wrapper.text()).toContain('Nothing matches these filters')
  await wrapper.get('[aria-label="Clear search and filters"]').trigger('click')
  expect(wrapper.findAll('.task-card')).toHaveLength(2)
  await wrapper.get('[aria-label="Kind"]').setValue('builtin')
  expect(wrapper.findAll('.task-card')).toHaveLength(0)
  wrapper.unmount()
})

it('counts each status tab against the other filters', async () => {
  tasks.value = [task(1, 'completed'), task(2, 'waiting'), task(3, 'failed')]
  const wrapper = mount(HistoryPanel)
  const counts = () => wrapper.get('[aria-label="Status"]').findAll('button').map(button => button.text())
  expect(counts()).toEqual(['All3', 'Needs you1', 'In progress0', 'Done1', "Didn't finish1"])
  await wrapper.get('[aria-label="Search history"]').setValue('vue')
  expect(counts()).toEqual(['All2', 'Needs you1', 'In progress0', 'Done0', "Didn't finish1"])
  wrapper.unmount()
})

it('keeps older tasks out of a recent date range', async () => {
  const now = Date.now() / 1000
  tasks.value = [{ ...task(1, 'completed'), createdAt: now }, { ...task(2, 'completed'), createdAt: now - 40 * 86400 }]
  const wrapper = mount(HistoryPanel)
  await wrapper.get('[aria-label="When"]').setValue('month')
  expect(wrapper.findAll('.task-card')).toHaveLength(1)
  expect(wrapper.get('.group-title').text()).toBe('Today')
  wrapper.unmount()
})

it('limits history rendering and reveals additional tasks on request', async () => {
  tasks.value = Array.from({ length: 12 }, (_, index) => task(index + 1, 'completed'))
  const wrapper = mount(HistoryPanel)
  expect(wrapper.findAll('.task-card')).toHaveLength(10)
  await wrapper.get('.load-more').trigger('click')
  expect(wrapper.findAll('.task-card')).toHaveLength(12)
  wrapper.unmount()
})

it('keeps task results off the assistant page; they live in history', () => {
  tasks.value = [task(1, 'completed'), task(2, 'running'), task(3, 'completed')]
  const wrapper = mount(AssistantPanel)
  expect(wrapper.findAll('.task-card')).toHaveLength(0)
  wrapper.unmount()
})

it('selects the supported agent mode automatically and keeps built-in actions local', async () => {
  connections.value.active.kind = 'codex'
  const wrapper = mount(AssistantPanel)
  expect(wrapper.find('.task-composer .task-problem').exists()).toBe(true)
  await wrapper.findAll('.task-shortcuts button').find(button => button.text() === 'Organise files')!.trigger('click')
  expect(wrapper.find('.task-composer .task-problem').exists()).toBe(false)
  expect(wrapper.get('.preset-pill').text()).toContain('runs on this computer')
  wrapper.unmount()
})

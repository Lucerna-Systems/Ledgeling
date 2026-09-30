// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import AssistantPanel from './AssistantPanel.vue'
import { snapshot } from '../state'
import { assignedPet, connections, providerTerms } from './state'
import type { Connections, ProviderSetup } from './types'
import type { Snapshot } from '../types'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: api.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const setup = (model: string, over: Partial<ProviderSetup> = {}): ProviderSetup => ({ model, endpoint: '', executable: '', termsVersion: 1, acceptedAt: 1, ...over })

beforeEach(() => {
  assignedPet.value = null
  providerTerms.value = {}
  snapshot.value = {
    settings: { paused: false, hidden: false, speedScale: 1 }, notes: [], packs: [], perchCount: 0,
    pets: [{ id: 1, packId: 'pip', name: 'Pip', class: 'flier', state: 'rest', energy: 1, mood: .7, surface: null }],
  } as unknown as Snapshot
  connections.value = {
    active: { kind: 'ollama', model: 'llama', endpoint: 'http://127.0.0.1:11434', executable: '' }, searchEnabled: false, perPet: {},
    providers: { ollama: setup('llama', { endpoint: 'http://127.0.0.1:11434', termsVersion: 0 }), apple: setup('system') },
  }
  // The backend's answer is what the picker shows, so the mock echoes the save.
  api.invoke.mockReset().mockImplementation(async (command: string, args?: { config?: Connections }) => {
    if (command === 'task_save_connections') return args!.config
    if (command === 'task_provider_terms') return { apple: { version: 1, text: 'terms' } }
    return null
  })
})

async function openPicker() {
  const wrapper = mount(AssistantPanel)
  await wrapper.get('.model-chip').trigger('click')
  const rows = async () => {
    await wrapper.get('.model-provider .dropdown-button').trigger('click')
    return wrapper.findAll('.model-provider li')
  }
  return { wrapper, rows }
}

it('offers only the providers that have been added', async () => {
  const { wrapper, rows } = await openPicker()
  expect((await rows()).map(li => li.text())).toEqual(['No AI · built-in actions only', 'Apple Intelligence', 'Ollama'])
  expect(wrapper.find('.model-list').exists()).toBe(false)
  wrapper.unmount()
})

it('picking a provider saves it, and the chip shows what was saved', async () => {
  const { wrapper, rows } = await openPicker()
  await (await rows()).find(li => li.text() === 'Apple Intelligence')!.trigger('click')
  await flushPromises()
  const [, args] = api.invoke.mock.calls.find(([command]) => command === 'task_save_connections')!
  expect((args as { config: Connections }).config.active).toEqual({ kind: 'apple', model: 'system', endpoint: '', executable: '' })
  expect(connections.value.active.kind).toBe('apple')
  expect(wrapper.get('.model-chip').text()).toBe('Apple Intelligence')
  // No per-request consent: it was given when the provider was added.
  expect(wrapper.text()).not.toContain('read this request')
  wrapper.unmount()
})

it('a pick the backend refuses never shows as chosen', async () => {
  api.invoke.mockImplementation(async (command: string) => {
    if (command === 'task_save_connections') throw 'Set up Apple Intelligence on the AI providers page first.'
    return null
  })
  const { wrapper, rows } = await openPicker()
  await (await rows()).find(li => li.text() === 'Apple Intelligence')!.trigger('click')
  await flushPromises()
  expect(connections.value.active.kind).toBe('ollama')
  expect(wrapper.get('.model-chip').text()).toBe('Ollama')
  wrapper.unmount()
})

it('a pet can follow the default or carry its own provider', async () => {
  connections.value.perPet = { 1: { kind: 'apple', model: 'system', endpoint: '', executable: '' } }
  const { wrapper, rows } = await openPicker()
  await wrapper.findAll('.pet-choice').find(choice => choice.text().includes('Pip'))!.trigger('click')
  expect(wrapper.get('.model-chip').text()).toBe('Apple Intelligence')
  await wrapper.get('.model-chip').trigger('click')
  await (await rows()).find(li => li.text() === 'Default · Ollama')!.trigger('click')
  await flushPromises()
  expect(connections.value.perPet[1]).toBeUndefined()
  expect(wrapper.get('.model-chip').text()).toBe('Ollama')
  wrapper.unmount()
})

it('an added provider still waiting on its terms holds the request and says where to fix it', async () => {
  connections.value.active = { kind: 'apple', model: 'system', endpoint: '', executable: '' }
  connections.value.providers.apple = setup('system', { termsVersion: 0 })
  providerTerms.value = { apple: { version: 1, text: 'terms' } }
  const wrapper = mount(AssistantPanel)
  await flushPromises()
  await wrapper.get('#task-prompt').setValue('Tidy up')
  expect(wrapper.get('.task-composer .task-problem').text()).toContain('Apple Intelligence needs its terms accepted')
  expect(wrapper.get('.send-button').attributes('disabled')).toBeDefined()
  await wrapper.get('.task-composer .task-problem .text-link').trigger('click')
  expect(wrapper.emitted('providers')).toHaveLength(1)
  wrapper.unmount()
})

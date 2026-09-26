// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import ConnectionsPanel from '../assistant/ConnectionsPanel.vue'
import { snapshot } from '../state'
import { connections } from '../assistant/state'
import type { Snapshot } from '../types'

const invoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: (...args: unknown[]) => invoke(...args) }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))
const detection = { codexInstalled: true, claudeCode: false, ollamaApp: false, ollamaReachable: false, openaiKey: true, anthropicKey: false, compatibleKey: false, braveKey: false }

beforeEach(() => {
  invoke.mockReset()
  invoke.mockImplementation(async (command: string) => {
    if (command === 'task_detect_providers') return detection
    if (command === 'task_models') return ['codex-mini', 'gpt-5.2-codex']
    if (command === 'task_account') return { account: { email: 'dirk@example.com' } }
    return null
  })
  connections.value = { active: { kind: 'none', model: '', endpoint: '', executable: '' }, searchEnabled: false, perPet: {}, appleTerms: { acceptedVersion: 0, acceptedAt: 0 } }
  snapshot.value = null
})

it('offers only detected installed apps and reports saved keys in the API select', async () => {
  const wrapper = mount(ConnectionsPanel)
  await flushPromises()
  const selects = wrapper.findAll('select')
  expect((selects[0].element as HTMLSelectElement).textContent).toContain('ChatGPT · Codex')
  expect((selects[0].element as HTMLSelectElement).textContent).not.toContain('Ollama')
  expect((selects[1].element as HTMLSelectElement).textContent).toContain('OpenAI · key saved')
  expect((selects[1].element as HTMLSelectElement).textContent).not.toContain('key saved</option><option value="anthropic">Anthropic')
  wrapper.unmount()
})

it('choosing an installed app switches the default and loads its model list', async () => {
  const wrapper = mount(ConnectionsPanel)
  await flushPromises()
  await wrapper.findAll('select')[0].setValue('codex')
  await flushPromises()
  expect(connections.value.active.kind).toBe('codex')
  expect(connections.value.active.executable).toBe('codex')
  const call = invoke.mock.calls.find(([command]) => command === 'task_models')!
  expect((call[1] as { config: { kind: string } }).config.kind).toBe('codex')
  expect(wrapper.text()).toContain('Signed in to ChatGPT as dirk@example.com.')
  wrapper.unmount()
})

it('saves a per-companion override immediately when one is picked', async () => {
  snapshot.value = {
    settings: { paused: false, hidden: false, speedScale: 1 }, notes: [], packs: [], perchCount: 0,
    pets: [{ id: 3, packId: 'pip', name: 'Pip', class: 'flier', state: 'rest', energy: 1, mood: .7, surface: null }],
  } as unknown as Snapshot
  const wrapper = mount(ConnectionsPanel)
  await flushPromises()
  await wrapper.get('.per-pet-summary button').trigger('click')
  await flushPromises()
  await wrapper.get('.per-pet-edit select').setValue('openai')
  await flushPromises()
  const input = wrapper.get('.per-pet-edit input')
  await input.setValue('gpt-5.2')
  await input.trigger('keydown', { key: 'Enter' })
  await wrapper.get('.per-pet-edit .primary-button').trigger('click')
  await flushPromises()
  expect(connections.value.perPet[3]).toEqual({ kind: 'openai', model: 'gpt-5.2', endpoint: '', executable: '' })
  expect(invoke.mock.calls.some(([command]) => command === 'task_save_connections')).toBe(true)
  wrapper.unmount()
})
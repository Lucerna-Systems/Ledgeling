// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import PacksPanel from './PacksPanel.vue'
import { snapshot } from '../state'
import type { Snapshot } from '../types'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const settings = { version: 1, pets: [], tipsDismissed: false, welcomed: true }

beforeEach(() => {
  api.invoke.mockReset().mockImplementation(async (command: string, args: { settings?: unknown }) => {
    if (command === 'list_packs') return []
    if (command === 'set_settings') return { settings: args.settings, pets: [], packs: [], notes: [] }
    return undefined
  })
  snapshot.value = { settings: { ...settings }, pets: [], packs: [], notes: [] } as unknown as Snapshot
})

it('shows the first-run tip until it is dismissed, then saves that', async () => {
  const wrapper = mount(PacksPanel)
  await flushPromises()
  expect(wrapper.find('.tip-card').text()).toContain('Meet your pet')
  expect(wrapper.find('.tip-card').text()).toContain('pet it')
  await wrapper.get('[aria-label="Dismiss tip"]').trigger('click')
  await flushPromises()
  expect(api.invoke).toHaveBeenCalledWith('set_settings', { settings: { ...settings, tipsDismissed: true } })
  expect(wrapper.find('.tip-card').exists()).toBe(false)
  wrapper.unmount()
})

it('does not show the tip once it was dismissed before', async () => {
  snapshot.value = { settings: { ...settings, tipsDismissed: true }, pets: [], packs: [], notes: [] } as unknown as Snapshot
  const wrapper = mount(PacksPanel)
  await flushPromises()
  expect(wrapper.find('.tip-card').exists()).toBe(false)
  wrapper.unmount()
})

it('renames a pet that is out with the trimmed name', async () => {
  const summary = { id: 'pip', name: 'Pip', author: 'a', version: '1', description: '', license: '', tags: [], class: 'flier', surfaces: [], animations: [], assistantClips: [], preview: null, directory: '', builtin: true, errors: 0, warnings: 0, notes: 0, loadable: true }
  api.invoke.mockImplementation(async (command: string, args: { settings?: unknown }) => {
    if (command === 'list_packs') return [{ summary, report: { diagnostics: [] } }]
    if (command === 'set_settings') return { settings: args.settings, pets: [], packs: [], notes: [] }
    return undefined
  })
  snapshot.value = { settings: { ...settings, tipsDismissed: true }, packs: [], notes: [], pets: [{ id: 7, packId: 'pip', name: 'Pip', days: 3, affection: 0.4 }] } as unknown as Snapshot
  const wrapper = mount(PacksPanel)
  await flushPromises()
  expect(wrapper.find('.bond-days').text()).toBe('Together 3 days')
  const input = wrapper.get('input.pet-name')
  await input.setValue('  Biscuit  ')
  await input.trigger('change')
  await flushPromises()
  expect(api.invoke).toHaveBeenCalledWith('rename_pet', { petId: 7, name: 'Biscuit' })
  wrapper.unmount()
})

it('makes a biome a pet\'s home through set_pet_home', async () => {
  const summary = { id: 'pip', name: 'Pip', author: 'a', version: '1', description: '', license: '', tags: [], class: 'flier', surfaces: [], animations: [], assistantClips: [], preview: null, directory: '', builtin: true, errors: 0, warnings: 0, notes: 0, loadable: true }
  api.invoke.mockImplementation(async (command: string) => {
    if (command === 'list_packs') return [{ summary, report: { diagnostics: [] } }]
    return undefined
  })
  snapshot.value = { settings: { ...settings, tipsDismissed: true, biomes: [{ id: 4, app: 'safari', props: [] }] }, packs: [], notes: [], pets: [{ id: 7, packId: 'pip', name: 'Pip', days: 3, affection: 0.4, homeBiome: null }] } as unknown as Snapshot
  const wrapper = mount(PacksPanel)
  await flushPromises()
  const home = wrapper.get('select.pet-home')
  expect(home.text()).toContain('Home: safari')
  await home.setValue('4')
  await flushPromises()
  expect(api.invoke).toHaveBeenCalledWith('set_pet_home', { pet: 7, biome: 4 })
  wrapper.unmount()
})

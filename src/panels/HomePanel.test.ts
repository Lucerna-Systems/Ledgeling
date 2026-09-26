// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import HomePanel from './HomePanel.vue'
import { packs, snapshot } from '../state'
import type { PackListing, Snapshot } from '../types'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const settings = { paused: false, hidden: false, speedScale: 1.3 }
const pet = { id: 1, packId: 'pip', name: 'Pip the Budgie', class: 'flier', state: 'rest', energy: 1, mood: 1, surface: null }

beforeEach(() => {
  api.invoke.mockReset().mockResolvedValue(undefined)
  packs.value = []
  snapshot.value = { settings: { ...settings }, pets: [pet], packs: [] } as unknown as Snapshot
})

it('shows the nearest pace preset and saves the one picked', async () => {
  const wrapper = mount(HomePanel)
  const buttons = wrapper.get('[aria-label="Pace"]').findAll('button')
  expect(buttons.map(button => button.attributes('aria-checked'))).toEqual(['false', 'true', 'false'])
  await buttons[2].trigger('click')
  expect(api.invoke.mock.calls[0]).toEqual(['set_settings', { settings: { ...settings, speedScale: 1.8 } }])
  wrapper.unmount()
})

it('offers pets to bring out when nobody is on the desktop', async () => {
  snapshot.value = { settings: { ...settings }, pets: [], packs: [] } as unknown as Snapshot
  packs.value = [{ summary: { id: 'pip', name: 'Pip', preview: null, loadable: true }, report: { diagnostics: [] } } as unknown as PackListing]
  const wrapper = mount(HomePanel)
  expect(wrapper.find('[aria-label="Pace"]').exists()).toBe(false)
  await wrapper.get('.starter-list button').trigger('click')
  expect(api.invoke.mock.calls[0]).toEqual(['spawn_pet', { packId: 'pip' }])
  wrapper.unmount()
})

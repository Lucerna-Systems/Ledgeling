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

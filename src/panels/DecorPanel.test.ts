// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import DecorPanel from './DecorPanel.vue'
import { snapshot } from '../state'
import type { Snapshot } from '../types'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const meadow = {
  id: 'com.perch.decor-meadow', name: 'Meadow', author: 'Ledgeling', builtin: true, usedBy: 1,
  props: [{ name: 'oak', image: 'data:image/png;base64,', frames: 1, width: 48, height: 64, animations: ['breeze', 'rustle'], perch: true }],
}
const garden = { ...meadow, id: 'com.studio.decor-garden', name: 'Garden', author: 'Me', builtin: false, usedBy: 0 }

beforeEach(() => {
  api.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'list_decor') return [meadow, garden]
    return undefined
  })
  snapshot.value = {
    settings: { biomes: [{ id: 1, app: 'safari', props: [] }, { id: 2, app: 'slack', props: [] }] },
    openBiomes: [1],
  } as unknown as Snapshot
})

describe('decor page', () => {
  it('lists every decor pack with what it animates and where it is used', async () => {
    const wrapper = mount(DecorPanel)
    await flushPromises()
    const cards = wrapper.findAll('.decor-card')
    expect(cards).toHaveLength(2)
    expect(cards[0].text()).toContain('comes with Ledgeling')
    expect(cards[0].text()).toContain('in 1 biome')
    expect(cards[0].text()).toContain('Breeze, Rustle')
    wrapper.unmount()
  })

  it('offers a copy or the original for bundled decor, and deletes only your own after a second click', async () => {
    const wrapper = mount(DecorPanel)
    await flushPromises()
    const [bundled, own] = wrapper.findAll('.decor-card')
    expect(bundled.text()).toContain('Edit a copy')
    expect(bundled.text()).toContain('Edit original')
    expect(bundled.text()).not.toContain('Delete')
    const remove = own.get('[aria-label="Delete Garden"]')
    await remove.trigger('click')
    expect(api.invoke).not.toHaveBeenCalledWith('delete_decor', expect.anything())
    await remove.trigger('click')
    expect(api.invoke).toHaveBeenCalledWith('delete_decor', { id: 'com.studio.decor-garden' })
    wrapper.unmount()
  })

  it('decorates an open window straight away and greys out a closed one', async () => {
    const wrapper = mount(DecorPanel)
    await flushPromises()
    await wrapper.findAll('[role="radio"]')[1].trigger('click')
    const rows = wrapper.findAll('.biome-row')
    expect(rows[0].text()).toContain('open now')
    expect(rows[1].text()).toContain('not open')
    expect(rows[1].findAll('button').at(-1)!.attributes('disabled')).toBeDefined()
    await rows[0].findAll('button').at(-1)!.trigger('click')
    expect(api.invoke).toHaveBeenCalledWith('decorate_biome', { id: 1 })
    wrapper.unmount()
  })
})

// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import DecorateMode from './DecorateMode.vue'
import type { EditView } from '../types'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke }))

const palette = [
  { decor: 'meadow', prop: 'bush', width: 32, height: 20 },
  { decor: 'meadow', prop: 'oak', width: 48, height: 64 },
]
const editing: EditView = {
  picking: false, biome: 1, app: 'safari', edgeX: 100, edgeY: 400, edgeWidth: 600,
  props: [{ decor: 'meadow', prop: 'oak', x: 0.5, flip: false }], palette, error: null, notice: null,
}
const atlas = () => ({ image: 'data:image/png;base64,', frames: 1, fps: 0, width: 32, height: 20 })

beforeEach(() => { api.invoke.mockReset().mockResolvedValue(undefined) })

describe('decorate mode', () => {
  it('sends the clicked point while picking a window', async () => {
    const wrapper = mount(DecorateMode, { props: { edit: { ...editing, picking: true, props: [] }, atlas } })
    await wrapper.get('.decorate').trigger('pointerdown', { clientX: 320, clientY: 210, button: 0 })
    expect(api.invoke).toHaveBeenCalledWith('biome_pick', { point: [320, 210] })
    wrapper.unmount()
  })

  it('places a palette prop where it is dropped above the edge', async () => {
    const wrapper = mount(DecorateMode, { props: { edit: editing, atlas } })
    await wrapper.get('[aria-label="Add bush"]').trigger('pointerdown', { clientX: 10, clientY: 10, button: 0 })
    await wrapper.get('.decorate').trigger('pointerup', { clientX: 250, clientY: 390 })
    expect(api.invoke).toHaveBeenCalledWith('biome_decorate', { change: { kind: 'place', decor: 'meadow', prop: 'bush', x: 0.25 } })
    wrapper.unmount()
  })

  it('changes nothing when a prop is dropped away from the edge', async () => {
    const wrapper = mount(DecorateMode, { props: { edit: editing, atlas } })
    await wrapper.get('[aria-label="Add bush"]').trigger('pointerdown', { clientX: 10, clientY: 10, button: 0 })
    await wrapper.get('.decorate').trigger('pointerup', { clientX: 250, clientY: 900 })
    expect(api.invoke).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('moves, flips and removes a placed prop, and Done ends the mode', async () => {
    const wrapper = mount(DecorateMode, { props: { edit: editing, atlas } })
    const handle = wrapper.get('.decorate-handle')
    await handle.trigger('pointerdown', { clientX: 400, clientY: 380, button: 0 })
    await wrapper.get('.decorate').trigger('pointerup', { clientX: 700, clientY: 380 })
    await handle.trigger('dblclick')
    await handle.trigger('contextmenu')
    await wrapper.get('.decorate-done').trigger('click')
    expect(api.invoke.mock.calls).toEqual([
      ['biome_decorate', { change: { kind: 'move', index: 0, x: 1 } }],
      ['biome_decorate', { change: { kind: 'flip', index: 0 } }],
      ['biome_decorate', { change: { kind: 'remove', index: 0 } }],
      ['biome_decorate_end'],
    ])
    wrapper.unmount()
  })
})

it('says when props on this window are cut off by the top of the screen', () => {
  const wrapper = mount(DecorateMode, { props: { edit: { ...editing, notice: 'Allow Ledgeling under Accessibility' }, atlas } })
  expect(wrapper.get('.decorate-notice').text()).toContain('Accessibility')
  wrapper.unmount()
})

// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import DecorStudio from './DecorStudio.vue'
import { newProp, type DecorDraft } from './decorModel'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

beforeEach(() => {
  api.invoke.mockReset().mockResolvedValue('com.studio.decor-garden')
  HTMLCanvasElement.prototype.getContext = vi.fn(() => null) as never
  HTMLCanvasElement.prototype.toDataURL = () => 'data:,'
  HTMLCanvasElement.prototype.getBoundingClientRect = () => ({ left: 0, top: 0, width: 320, height: 320 }) as DOMRect
  HTMLCanvasElement.prototype.setPointerCapture = () => {}
})

function existing(): DecorDraft {
  const prop = newProp([], 8, 8)
  prop.name = 'shrub'
  prop.pixels.fill(255)
  return { id: 'com.studio.decor-garden', name: 'Garden', author: 'Ana', props: [prop] }
}
const saveArgs = () => api.invoke.mock.calls.find(c => c[0] === 'save_decor_pack')![1]

it('says what is missing, then saves a drawn prop as a decor pack', async () => {
  const wrapper = mount(DecorStudio)
  expect(wrapper.text()).toContain('Give the decor pack a name.')
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeDefined()
  await wrapper.get('[aria-label="Decor pack name"]').setValue('Garden')
  await wrapper.get('[aria-label="Prop name"]').setValue('shrub')
  // 32 × 32 over 320 px: paint the bottom-left pixel.
  await wrapper.get('.pixel-canvas').trigger('pointerdown', { button: 0, clientX: 5, clientY: 315, pointerId: 1 })
  await wrapper.get('.pixel-canvas').trigger('pointerup')
  expect(wrapper.text()).not.toContain('Before saving')
  await wrapper.get('.primary-button').trigger('click')
  await flushPromises()
  const args = saveArgs()
  const manifest = JSON.parse(args.manifest)
  expect(manifest.props.shrub).toMatchObject({ file: 'sprites/shrub.png', width: 32, height: 32, sway: 'soft' })
  expect(manifest.props.shrub.animations).toBeUndefined()
  expect(args.sprites[0].pixels[(31 * 32) * 4 + 3]).toBe(255)
  expect(args.inPlace).toBe(false)
  expect(wrapper.text()).toContain('is saved')
  expect(wrapper.emitted('saved')).toEqual([['com.studio.decor-garden']])
  wrapper.unmount()
})

it('draws a prop as one still image, with no frame strip or preview', () => {
  const wrapper = mount(DecorStudio)
  expect(wrapper.find('.pixel-canvas').exists()).toBe(true)
  expect(wrapper.find('.frame-strip').exists()).toBe(false)
  expect(wrapper.find('.preview-play').exists()).toBe(false)
  wrapper.unmount()
})

it('saves an edit in place under the original id, with the chosen animations', async () => {
  const wrapper = mount(DecorStudio, { props: { initial: existing(), mode: 'in_place' } })
  expect(wrapper.get('.primary-button').text()).toBe('Save changes')
  await wrapper.get('[aria-label="Breeze"]').setValue(true)
  await wrapper.get('[aria-label="Twinkle"]').setValue(true)
  await wrapper.get('.primary-button').trigger('click')
  await flushPromises()
  const args = saveArgs()
  expect(args.inPlace).toBe(true)
  const manifest = JSON.parse(args.manifest)
  expect(manifest.id).toBe('com.studio.decor-garden')
  expect(manifest.props.shrub.animations).toEqual(['breeze', 'twinkle'])
  wrapper.unmount()
})

it('saves a copy as a new pack and will not let it take the original id', async () => {
  const initial = existing()
  const wrapper = mount(DecorStudio, { props: { initial, mode: 'copy' } })
  expect((wrapper.get('[aria-label="Decor pack name"]').element as HTMLInputElement).value).toBe('Garden copy')
  await wrapper.get('[aria-label="Decor pack name"]').setValue('Garden')
  expect(wrapper.text()).toContain('name of its own')
  await wrapper.get('[aria-label="Decor pack name"]').setValue('Meadow')
  await wrapper.get('.primary-button').trigger('click')
  await flushPromises()
  const args = saveArgs()
  expect(args.inPlace).toBe(false)
  expect(JSON.parse(args.manifest).id).toBe('com.studio.decor-meadow')
  // The host's draft is left as it was.
  expect(initial.name).toBe('Garden')
  wrapper.unmount()
})

it('asks its host to close', async () => {
  const wrapper = mount(DecorStudio)
  await wrapper.findAll('button').find(b => b.text() === 'Back')!.trigger('click')
  expect(wrapper.emitted('close')).toHaveLength(1)
  wrapper.unmount()
})

it('shows the ledge over the canvas only while pets can sit on the prop', async () => {
  const wrapper = mount(DecorStudio)
  expect(wrapper.find('.decor-ledge').exists()).toBe(false)
  await wrapper.get('[aria-label="Pets can sit on it"]').setValue(true)
  const line = wrapper.get('.canvas-overlay .decor-ledge')
  expect(Number(line.attributes('y1'))).toBeCloseTo(3.2)
  wrapper.unmount()
})

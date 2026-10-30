// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import DecorStudio from './DecorStudio.vue'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

beforeEach(() => {
  api.invoke.mockReset().mockResolvedValue('com.studio.decor-garden')
  HTMLCanvasElement.prototype.getContext = vi.fn(() => null) as never
  HTMLCanvasElement.prototype.getBoundingClientRect = () => ({ left: 0, top: 0, width: 320, height: 320 }) as DOMRect
})

it('says what is missing, then saves a drawn prop as a decor pack', async () => {
  const wrapper = mount(DecorStudio)
  expect(wrapper.text()).toContain('Give the decor pack a name.')
  expect(wrapper.get('.primary-button').attributes('disabled')).toBeDefined()
  await wrapper.get('[aria-label="Decor pack name"]').setValue('Garden')
  await wrapper.get('[aria-label="Prop name"]').setValue('shrub')
  // 32 × 32 at 10 px a pixel: paint the bottom-left pixel.
  await wrapper.get('canvas').trigger('pointerdown', { button: 0, clientX: 5, clientY: 315 })
  await wrapper.get('canvas').trigger('pointerup')
  expect(wrapper.text()).not.toContain('Before saving')
  await wrapper.get('.primary-button').trigger('click')
  await flushPromises()
  const [command, args] = api.invoke.mock.calls.find(c => c[0] === 'save_decor_pack')!
  expect(command).toBe('save_decor_pack')
  const manifest = JSON.parse(args.manifest)
  expect(manifest.props.shrub).toMatchObject({ file: 'sprites/shrub.png', width: 32, height: 32, sway: 'soft' })
  expect(args.sprites[0].pixels[(31 * 32) * 4 + 3]).toBe(255)
  expect(wrapper.text()).toContain('is saved')
  wrapper.unmount()
})

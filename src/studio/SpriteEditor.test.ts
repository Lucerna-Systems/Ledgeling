// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, expect, it } from 'vitest'
import SpriteEditor from './SpriteEditor.vue'

const context = new Proxy({}, { get: () => () => {} })
globalThis.ImageData ??= class { constructor(public data: Uint8ClampedArray, public width: number, public height: number) {} } as never
beforeEach(() => {
  HTMLCanvasElement.prototype.getContext = (() => context) as never
  HTMLCanvasElement.prototype.toDataURL = () => 'data:,'
  HTMLCanvasElement.prototype.setPointerCapture = () => {}
})

// A tall prop, 10 wide and 20 high, shown at 10 screen pixels a pixel.
const W = 10, H = 20
function mountEditor(props: Record<string, unknown> = {}) {
  const frames = [new Uint8ClampedArray(W * H * 4)]
  const wrapper = mount(SpriteEditor, { props: { width: W, height: H, frames, ...props }, attachTo: document.body })
  const canvas = wrapper.get('.pixel-canvas')
  canvas.element.getBoundingClientRect = () => ({ left: 0, top: 0, width: W * 10, height: H * 10 }) as DOMRect
  return { wrapper, frames, canvas }
}
const alpha = (f: Uint8ClampedArray, x: number, y: number) => f[(y * W + x) * 4 + 3]

it('edits one still image of any size: no frame strip, onion skin or preview', async () => {
  const { wrapper, frames, canvas } = mountEditor()
  expect(wrapper.find('.frame-strip').exists()).toBe(false)
  expect(wrapper.find('[aria-label="Onion skin"]').exists()).toBe(false)
  expect(wrapper.find('.preview-play').exists()).toBe(false)
  expect(canvas.attributes('width')).toBe(String(W * 8))
  expect(canvas.attributes('height')).toBe(String(H * 8))
  // The bottom-right pixel of the tall image.
  await canvas.trigger('pointerdown', { button: 0, clientX: 95, clientY: 195, pointerId: 1 })
  await canvas.trigger('pointerup')
  expect(alpha(frames[0]!, 9, 19)).toBe(255)
  expect(wrapper.emitted('edited')).toBeTruthy()
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'z', metaKey: true }))
  expect(alpha(frames[0]!, 9, 19)).toBe(0)
  wrapper.unmount()
})

it('mirrors across the middle of a non-square image', async () => {
  const { wrapper, frames, canvas } = mountEditor()
  await wrapper.vm.$nextTick() // the keyboard listener is added once mounted
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'm' }))
  await canvas.trigger('pointerdown', { button: 0, clientX: 5, clientY: 5, pointerId: 1 })
  expect(alpha(frames[0]!, 0, 0)).toBe(255)
  expect(alpha(frames[0]!, W - 1, 0)).toBe(255)
  wrapper.unmount()
})

it('with animation on, adds frames of the same size and offers a preview', async () => {
  const { wrapper, frames } = mountEditor({ animation: true })
  expect(wrapper.find('.preview-play').exists()).toBe(true)
  await wrapper.get('[aria-label="Duplicate frame"]').trigger('click')
  expect(frames).toHaveLength(2)
  expect(frames[1]).toHaveLength(W * H * 4)
  expect(wrapper.findAll('.frame-thumb')).toHaveLength(2)
  wrapper.unmount()
})

it('shows the empty slot when there is nothing to draw on', () => {
  const wrapper = mount(SpriteEditor, { props: { width: W, height: H, frames: [] }, slots: { empty: '<p class="nothing">Start</p>' } })
  expect(wrapper.find('.nothing').exists()).toBe(true)
  expect(wrapper.find('.pixel-canvas').exists()).toBe(false)
  wrapper.unmount()
})

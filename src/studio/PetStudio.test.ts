// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import PetStudio from './PetStudio.vue'
import { draft } from './state'
import { FRAME, blankFrame, starterDraft } from './model'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

// jsdom has no canvas; the Studio only needs a context that accepts calls.
const context = new Proxy({}, { get: (_, key) => key === 'getImageData' ? () => ({ data: new Uint8ClampedArray(256 * 256 * 4) }) : () => {} })
HTMLCanvasElement.prototype.getContext = (() => context) as never
HTMLCanvasElement.prototype.toDataURL = () => 'data:,'
globalThis.ImageData ??= class { constructor(public data: Uint8ClampedArray, public width: number, public height: number) {} } as never

beforeEach(() => {
  api.invoke.mockReset().mockResolvedValue({ id: 'com.studio.ember', name: 'Ember' })
  draft.value = starterDraft()
})

it('will not save until the required animations are drawn', async () => {
  const wrapper = mount(PetStudio)
  expect(wrapper.text()).toContain('Draw at least one frame of the Idle animation')
  const save = wrapper.findAll('button').find(b => b.text() === 'Save')!
  expect(save.attributes('disabled')).toBeDefined()
  wrapper.unmount()
})

it('saves the design as a pack and can bring it straight out', async () => {
  const frame = blankFrame()
  frame.set([255, 0, 0, 255], (20 * FRAME + 20) * 4)
  draft.value.name = 'Ember'
  draft.value.clips.idle!.frames = [frame]
  draft.value.clips.move!.frames = [new Uint8ClampedArray(frame)]
  const wrapper = mount(PetStudio)
  await wrapper.findAll('button').find(b => b.text().includes('Save & bring out'))!.trigger('click')
  await vi.waitFor(() => expect(api.invoke.mock.calls.map(c => c[0])).toContain('spawn_pet'))
  const [command, args] = api.invoke.mock.calls[0]!
  expect(command).toBe('save_studio_pack')
  expect(JSON.parse(args.manifest).name).toBe('Ember')
  expect(args.strips.map((s: { name: string }) => s.name)).toEqual(['idle', 'move'])
  expect(args.preview).toMatchObject({ name: 'preview', width: 256, height: 256 })
  expect(api.invoke.mock.calls.find(c => c[0] === 'spawn_pet')![1]).toEqual({ packId: 'com.studio.ember' })
  expect(wrapper.text()).toContain('is saved and ready')
  wrapper.unmount()
})

it('the behaviour tab edits the same design', async () => {
  const wrapper = mount(PetStudio)
  await wrapper.findAll('[role=tab]')[1]!.trigger('click')
  const before = draft.value.states.length
  await wrapper.findAll('button').find(b => b.text().includes('Add a behaviour'))!.trigger('click')
  expect(draft.value.states.length).toBe(before + 1)
  expect(wrapper.findAll('.state-block')).toHaveLength(before + 1)
  wrapper.unmount()
})

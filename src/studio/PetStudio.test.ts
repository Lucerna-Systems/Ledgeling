// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import PetStudio from './PetStudio.vue'
import { draft } from './state'
import { FRAME, blankFrame, emptyClip, starterDraft } from './model'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))

// jsdom has no canvas; the Studio only needs a context that accepts calls.
const context = new Proxy({}, { get: (_, key) => key === 'getImageData' ? () => ({ data: new Uint8ClampedArray(256 * 256 * 4) }) : () => {} })
HTMLCanvasElement.prototype.getContext = (() => context) as never
HTMLCanvasElement.prototype.toDataURL = () => 'data:,'
globalThis.ImageData ??= class { constructor(public data: Uint8ClampedArray, public width: number, public height: number) {} } as never

beforeEach(() => {
  api.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'list_packs') return []
    if (command === 'export_pack') return { path: '/packs/ember.petpack', sha256: 'a'.repeat(64) }
    if (command === 'task_list') return []
    if (command === 'task_connections') return { active: { kind: 'none', model: '', endpoint: '', executable: '' }, searchEnabled: false, ollamaThinking: false, perPet: {}, providers: {} }
    if (command === 'task_provider_terms') return {}
    return { id: 'com.studio.ember', name: 'Ember' }
  })
  draft.value = starterDraft()
})

it('will not save until something is drawn', async () => {
  const wrapper = mount(PetStudio)
  expect(wrapper.text()).toContain('Draw at least one frame of any animation')
  const save = wrapper.findAll('button').find(b => b.text() === 'Save')!
  expect(save.attributes('disabled')).toBeDefined()
  wrapper.unmount()
})

it('saves the design as a pack and can bring it straight out', async () => {
  const frame = blankFrame()
  frame.set([255, 0, 0, 255], (20 * FRAME + 20) * 4)
  draft.value.name = 'Ember'
  draft.value.clips.idle!.frames = [frame]
  draft.value.clips.move = { ...emptyClip(), frames: [new Uint8ClampedArray(frame)] }
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

it('gives two pets with the same name their own ids, and saving again updates the same one', async () => {
  // The bug this guards: an unnamed or same-named pet replaced an earlier one,
  // and every pet already out of that kind changed with it.
  api.invoke.mockImplementation(async (command: string, args: { manifest?: string }) =>
    command === 'save_studio_pack' ? { id: JSON.parse(args.manifest!).id, name: 'Ember' } : command === 'list_packs' ? [] : undefined)
  const draw = () => {
    const frame = blankFrame()
    frame.set([255, 0, 0, 255], (20 * FRAME + 20) * 4)
    draft.value = starterDraft()
    draft.value.name = 'Ember'
    draft.value.clips.idle!.frames = [frame]
  }
  draw()
  const wrapper = mount(PetStudio)
  const save = () => wrapper.findAll('button').find(b => b.text() === 'Save')!.trigger('click')
  await save(); await flushPromises()
  await save(); await flushPromises()
  draw()
  await wrapper.vm.$nextTick()
  await save(); await flushPromises()
  const saves = api.invoke.mock.calls.filter(c => c[0] === 'save_studio_pack').map(c => c[1])
  const ids = saves.map(s => JSON.parse(s.manifest).id)
  expect(ids[0]).toMatch(/^com\.studio\.ember-[a-z0-9]{4}$/)
  expect(saves.map(s => s.replace)).toEqual([false, true, false])
  expect(ids[1]).toBe(ids[0])
  expect(ids[2]).not.toBe(ids[0])
  wrapper.unmount()
})

it('offers Export .petpack after a save and exports the saved pet', async () => {
  const frame = blankFrame()
  frame.set([255, 0, 0, 255], (20 * FRAME + 20) * 4)
  draft.value.name = 'Ember'
  draft.value.clips.idle!.frames = [frame]
  const wrapper = mount(PetStudio)
  expect(wrapper.text()).not.toContain('Export .petpack')
  await wrapper.findAll('button').find(b => b.text() === 'Save')!.trigger('click')
  await vi.waitFor(() => expect(wrapper.text()).toContain('Export .petpack'))
  await wrapper.findAll('button').find(b => b.text().includes('Export .petpack'))!.trigger('click')
  await vi.waitFor(() => expect(wrapper.text()).toContain('/packs/ember.petpack'))
  expect(api.invoke).toHaveBeenCalledWith('export_pack', { id: 'com.studio.ember' })
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

it('keeps the preview still until Play is pressed', async () => {
  const frames = vi.spyOn(globalThis, 'requestAnimationFrame').mockReturnValue(1)
  draft.value.clips.idle = { frames: [blankFrame(), blankFrame()], fps: 6, loop: true }
  const wrapper = mount(PetStudio)
  await wrapper.vm.$nextTick()
  expect(frames).not.toHaveBeenCalled()
  const play = wrapper.get('.preview-play')
  expect(play.text()).toBe('Play')
  await play.trigger('click')
  expect(frames).toHaveBeenCalled()
  expect(play.text()).toBe('Pause')
  await play.trigger('click')
  expect(play.text()).toBe('Play')
  wrapper.unmount()
  frames.mockRestore()
})

it('clears the "draw something" note as soon as anything is drawn', async () => {
  const wrapper = mount(PetStudio)
  expect(wrapper.text()).toContain('Draw at least one frame of any animation')
  const canvas = wrapper.get('.pixel-canvas')
  canvas.element.getBoundingClientRect = () => ({ left: 0, top: 0, width: 480, height: 480 }) as DOMRect
  canvas.element.setPointerCapture = () => {}
  await canvas.trigger('pointerdown', { button: 0, clientX: 240, clientY: 240, pointerId: 1 })
  expect(wrapper.text()).not.toContain('Draw at least one frame of any animation')
  wrapper.unmount()
})

it('selects with the magic wand, deletes the selection and undoes it', async () => {
  const wrapper = mount(PetStudio, { attachTo: document.body })
  const canvas = wrapper.get('.pixel-canvas')
  canvas.element.getBoundingClientRect = () => ({ left: 0, top: 0, width: 480, height: 480 }) as DOMRect
  canvas.element.setPointerCapture = () => {}
  const at = { button: 0, clientX: 240, clientY: 240, pointerId: 1 }
  await canvas.trigger('pointerdown', at)
  await canvas.trigger('pointerup', at)
  const drawnPixels = () => draft.value.clips.idle!.frames[0]!.some((v, i) => i % 4 === 3 && v > 0)
  expect(drawnPixels()).toBe(true)
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'w' }))
  await canvas.trigger('pointerdown', at)
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete' }))
  expect(drawnPixels()).toBe(false)
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'z', metaKey: true }))
  expect(drawnPixels()).toBe(true)
  wrapper.unmount()
})

it('zooms the canvas with the scroll wheel and fits again on request', async () => {
  const wrapper = mount(PetStudio, { attachTo: document.body })
  const readout = () => wrapper.get('.zoom-readout').text()
  await wrapper.vm.$nextTick()
  expect(readout()).toBe('100%')
  wrapper.get('.canvas-stage').element.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, clientX: 100, clientY: 100, cancelable: true }))
  await wrapper.vm.$nextTick()
  expect(parseInt(readout())).toBeGreaterThan(100)
  await wrapper.get('.zoom-readout').trigger('click')
  expect(readout()).toBe('100%')
  wrapper.unmount()
})

it('drafts behaviours from a description through the assistant and can undo', async () => {
  const { tasks, connections } = await import('../assistant/state')
  connections.value = { ...connections.value, active: { kind: 'ollama', model: 'm', endpoint: '', executable: '' } }
  const fallback = api.invoke.getMockImplementation()!
  api.invoke.mockImplementation(async (command: string, args: unknown) => command === 'task_start' ? { id: 5 } : fallback(command, args))
  const wrapper = mount(PetStudio)
  await wrapper.findAll('[role=tab]')[1]!.trigger('click')
  await wrapper.get('.ai-form input').setValue('hide from games')
  await wrapper.get('.ai-form').trigger('submit')
  await vi.waitFor(() => expect(api.invoke.mock.calls.find(c => c[0] === 'task_start')).toBeTruthy())
  const sent = api.invoke.mock.calls.find(c => c[0] === 'task_start')![1].request
  expect(sent).toMatchObject({ roots: [], mode: 'standard', petId: null })
  expect(sent.prompt).toContain('hide from games')
  tasks.value = [{ id: 5, status: 'completed', progress: '', error: null, result: '{"initial":"lurk","states":[{"name":"lurk","clip":"idle","next":[]}],"reactions":[]}' } as never]
  await vi.waitFor(() => expect(draft.value.states.map(s => s.name)).toEqual(['lurk']))
  await wrapper.findAll('button').find(b => b.text().includes('Undo'))!.trigger('click')
  expect(draft.value.states).toHaveLength(4)
  tasks.value = []
  wrapper.unmount()
})

it('undo from another animation goes back to the one it changes', async () => {
  const wrapper = mount(PetStudio, { attachTo: document.body })
  const canvas = wrapper.get('.pixel-canvas')
  canvas.element.getBoundingClientRect = () => ({ left: 0, top: 0, width: 480, height: 480 }) as DOMRect
  canvas.element.setPointerCapture = () => {}
  await canvas.trigger('pointerdown', { button: 0, clientX: 240, clientY: 240, pointerId: 1 })
  await canvas.trigger('pointerup')
  await wrapper.findAll('.clip-row').find(b => b.text().includes('Move'))!.trigger('click')
  expect(wrapper.find('.clip-start').exists()).toBe(true)
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'z', metaKey: true }))
  await vi.waitFor(() => expect(wrapper.get('.clip-row.selected').text()).toContain('Idle'))
  expect(draft.value.clips.idle!.frames[0]!.some((v, i) => i % 4 === 3 && v > 0)).toBe(false)
  wrapper.unmount()
})

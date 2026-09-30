// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import PetBubble from './PetBubble.vue'
import type { Task } from '../assistant/types'

const api = vi.hoisted(() => ({
  invoke: vi.fn(),
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    api.handlers.set(name, handler)
    return () => api.handlers.delete(name)
  }),
}))

const emit = async (name: string, payload: unknown) => {
  api.handlers.get(name)?.({ payload })
  await flushPromises()
}
const sent = () => api.invoke.mock.calls.filter(([command]) => command === 'pet_bubble').map(([, args]) => (args as { event: Record<string, unknown> }).event)
const show = (over = {}) => ({ session: 1, petId: 3, name: 'Budgie', runtime: 'none', model: '', folder: '/tmp/work', tailX: 80, tailUp: false, ...over })
const task = (over: Partial<Task> = {}): Task => ({
  id: 7, request: { prompt: 'p', petId: 3, roots: [], attachments: [], preset: '', mode: 'standard', parentId: null, allowCloud: false, allowWeb: false },
  connection: { kind: 'none', model: '', endpoint: '', executable: '' }, status: 'running', progress: 'Sorting files', result: '', error: null,
  approval: null, operations: [], sources: [], createdAt: 0, pet: null, title: '', ...over,
})
const button = (wrapper: ReturnType<typeof mount>, text: string) => wrapper.findAll('button').find(b => b.text() === text)

describe('the pet bubble', () => {
  beforeEach(() => {
    api.invoke.mockReset().mockResolvedValue(undefined)
    api.handlers.clear()
    vi.useFakeTimers()
  })
  afterEach(() => vi.useRealTimers())

  async function opened(over = {}) {
    const wrapper = mount(PetBubble, { attachTo: document.body })
    await flushPromises()
    await emit('bubble:show', show(over))
    return wrapper
  }

  it('asks for the opening once it can listen, and names the pet', async () => {
    const wrapper = await opened()
    expect(sent()[0]).toEqual({ type: 'ready' })
    expect(wrapper.text()).toContain('Ask Budgie')
    wrapper.unmount()
  })

  it('runs a built-in action in the bound folder with the chosen preset', async () => {
    const wrapper = await opened()
    await button(wrapper, 'Run action')!.trigger('click')
    expect(sent()).toContainEqual({ type: 'start', petId: 3, prompt: '', preset: 'sort_type', mode: 'actions', allowCloud: false, allowWeb: false, parentId: null })
    wrapper.unmount()
  })

  it('says what is missing instead of letting a run fail', async () => {
    const wrapper = await opened({ folder: '' })
    expect(wrapper.find('.hint').text()).toContain('Choose a folder')
    expect(button(wrapper, 'Run action')!.attributes('disabled')).toBeDefined()
    await emit('bubble:update', { petId: 3, folder: '/tmp/picked', message: 'Ready in /tmp/picked.' })
    expect(wrapper.find('.hint').exists()).toBe(false)
    expect(wrapper.text()).toContain('Ready in /tmp/picked.')
    wrapper.unmount()
  })

  it('shows progress, blocks a second run, and cancels without closing', async () => {
    const wrapper = await opened()
    await emit('bubble:update', task())
    expect(wrapper.text()).toContain('Sorting files')
    expect(button(wrapper, 'Run action')!.attributes('disabled')).toBeDefined()
    await button(wrapper, 'Cancel task')!.trigger('click')
    expect(sent()).toContainEqual({ type: 'cancel', id: 7 })
    expect(sent().some(e => e.type === 'closed')).toBe(false)
    wrapper.unmount()
  })

  it('ignores another pet’s task', async () => {
    const wrapper = await opened()
    await emit('bubble:update', task({ request: { ...task().request, petId: 9 } }))
    expect(wrapper.text()).not.toContain('Sorting files')
    wrapper.unmount()
  })

  it('answers the exact approval in plain words, never the payload', async () => {
    const wrapper = await opened()
    await emit('bubble:update', task({
      status: 'waiting',
      approval: { id: 42, title: 'Review agent command', details: { request: { threadId: 'th_01' } }, summary: { headline: 'Run a command', items: ['rm -rf build'], more: 2, note: 'Commands cannot always be undone.', accept: 'Allow', monospace: true } },
    }))
    const text = wrapper.text()
    expect(text).toContain('Run a command')
    expect(text).toContain('rm -rf build')
    expect(text).toContain('+ 2 more')
    expect(text).not.toContain('th_01')
    expect(wrapper.find('.output.mono').exists()).toBe(true)
    await button(wrapper, 'Allow')!.trigger('click')
    expect(sent()).toContainEqual({ type: 'approve', id: 7, approvalId: 42, accept: true })
    expect(button(wrapper, 'Reject')!.attributes('disabled')).toBeDefined()
    wrapper.unmount()
  })

  it('offers undo after applied changes and follows up on the finished task', async () => {
    const wrapper = await opened()
    await emit('bubble:update', task({ status: 'completed', result: 'Done.', operations: [{ kind: 'move', source: '/a', destination: '/b', content: null, state: 'applied' }] }))
    await button(wrapper, 'Undo')!.trigger('click')
    expect(sent()).toContainEqual({ type: 'undo', id: 7 })
    await button(wrapper, 'Run action')!.trigger('click')
    expect(sent().find(e => e.type === 'start')).toMatchObject({ parentId: 7 })
    wrapper.unmount()
  })

  it('swaps the model link for a picker and saves the choice', async () => {
    const wrapper = await opened({ runtime: 'ollama', model: 'llama3' })
    await button(wrapper, 'Select another model')!.trigger('click')
    expect(sent()).toContainEqual({ type: 'models', petId: 3 })
    expect(button(wrapper, 'Loading models…')!.attributes('disabled')).toBeDefined()
    await emit('bubble:update', { petId: 3, models: ['llama3', 'qwen3'] })
    await wrapper.find('.model-select button').trigger('click')
    await wrapper.findAll('.model-select li')[1].trigger('click')
    expect(sent()).toContainEqual({ type: 'model', petId: 3, model: 'qwen3' })
    wrapper.unmount()
  })

  it('closes on Escape by reporting the close of this opening, after the fade', async () => {
    const wrapper = await opened()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    expect(sent().some(e => e.type === 'closed')).toBe(false)
    vi.advanceTimersByTime(200)
    expect(sent()).toContainEqual({ type: 'closed', session: 1, petId: 3 })
    wrapper.unmount()
  })

  it('a dismissal meant for an earlier opening leaves this one open', async () => {
    const wrapper = await opened({ session: 2 })
    await emit('bubble:dismiss', 1)
    vi.advanceTimersByTime(200)
    expect(sent().some(e => e.type === 'closed')).toBe(false)
    await emit('bubble:dismiss', 2)
    vi.advanceTimersByTime(200)
    expect(sent()).toContainEqual({ type: 'closed', session: 2, petId: 3 })
    wrapper.unmount()
  })

  it('re-opening during the fade keeps the bubble and its pet', async () => {
    const wrapper = await opened()
    wrapper.find('[aria-label="Close"]').trigger('click')
    await emit('bubble:show', show({ session: 2 }))
    vi.advanceTimersByTime(200)
    expect(sent().some(e => e.type === 'closed')).toBe(false)
    wrapper.unmount()
  })

  it('closes at once when reduced motion is asked for', async () => {
    const original = window.matchMedia
    window.matchMedia = ((query: string) => ({ matches: query.includes('reduce'), addEventListener() {}, removeEventListener() {} })) as unknown as typeof window.matchMedia
    try {
      const wrapper = await opened()
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
      vi.advanceTimersByTime(0)
      expect(sent()).toContainEqual({ type: 'closed', session: 1, petId: 3 })
      wrapper.unmount()
    } finally {
      window.matchMedia = original
    }
  })

  it('switching to Ask keeps the folder it shares, hides the action menu and offers web research', async () => {
    const wrapper = await opened({ runtime: 'ollama' })
    expect(wrapper.find('.folder-row').exists()).toBe(true)
    await wrapper.find('[role="radio"][aria-checked="false"]').trigger('click')
    expect(wrapper.find('.folder-row').exists()).toBe(true)
    expect(wrapper.find('.action-select').exists()).toBe(false)
    expect(wrapper.text()).toContain('Allow web research')
    wrapper.unmount()
  })
})

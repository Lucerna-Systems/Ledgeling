// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import Companion from './Companion.vue'
import { snapshot } from './state'
import type { Snapshot } from './types'

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => false, invoke: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

it('preserves a request draft when navigating between companion pages', async () => {
  const wrapper = mount(Companion, { attachTo: document.body })
  const navigate = async (name: string) => {
    await wrapper.findAll('nav button').find(button => button.text() === name)!.trigger('click')
  }
  await navigate('Assistant')
  await wrapper.get('#task-prompt').setValue('Summarise my notes')
  await navigate('History')
  expect(wrapper.get('#task-prompt').isVisible()).toBe(false)
  await navigate('Assistant')
  expect(wrapper.get('#task-prompt').isVisible()).toBe(true)
  expect((wrapper.get('#task-prompt').element as HTMLTextAreaElement).value).toBe('Summarise my notes')
  wrapper.unmount()
})

it('resets page scroll when changing sections', async () => {
  const wrapper = mount(Companion)
  const main = wrapper.get('main').element
  main.scrollTop = 400
  await wrapper.findAll('nav button').find(button => button.text() === 'History')!.trigger('click')
  await Promise.resolve()
  expect(main.scrollTop).toBe(0)
  expect(wrapper.findAll('nav button').find(button => button.text() === 'History')!.attributes('aria-current')).toBe('page')
  wrapper.unmount()
})

it('keeps Settings selected on an advanced page and returns to it', async () => {
  const wrapper = mount(Companion)
  const nav = (name: string) => wrapper.findAll('nav button').find(button => button.text() === name)!
  await nav('Settings').trigger('click')
  await wrapper.findAll('.link-row').find(row => row.text().includes('Desktop map'))!.trigger('click')
  expect(wrapper.text()).toContain('Desktop view')
  expect(nav('Settings').attributes('aria-current')).toBe('page')
  await wrapper.get('.back-link').trigger('click')
  expect(wrapper.find('.link-row').exists()).toBe(true)
  wrapper.unmount()
})

it('opens on the Assistant and offers pause and hide in the sidebar while pets are out', () => {
  snapshot.value = {
    settings: { paused: false, hidden: false, speedScale: 1 }, notes: [], packs: [], perchCount: 0,
    pets: [{ id: 1, packId: 'pip', name: 'Pip', class: 'flier', state: 'perch', energy: 1, mood: .7, surface: 'window_top' }],
  } as unknown as Snapshot
  const wrapper = mount(Companion)
  expect(wrapper.get('nav button[aria-current="page"]').text()).toBe('Assistant')
  expect(wrapper.findAll('.sidebar-controls button').map(button => button.text())).toEqual(['Pause', 'Hide'])
  wrapper.unmount()
  snapshot.value = null
})

it('opens the Pet Studio from Pets and comes back to Pets', async () => {
  // jsdom has no canvas; the Studio only needs something that accepts drawing calls.
  HTMLCanvasElement.prototype.getContext = (() => new Proxy({}, { get: () => () => {} })) as never
  globalThis.ImageData ??= class { constructor(public data: Uint8ClampedArray, public width: number, public height: number) {} } as never
  const wrapper = mount(Companion)
  const nav = (name: string) => wrapper.findAll('nav button').find(button => button.text() === name)!
  await nav('Pets').trigger('click')
  await wrapper.get('.make-pet').trigger('click')
  expect(wrapper.find('.studio').exists()).toBe(true)
  expect(nav('Pets').attributes('aria-current')).toBe('page')
  expect(wrapper.get('.back-link').text()).toBe('Pets')
  await wrapper.get('.back-link').trigger('click')
  expect(wrapper.find('.studio').exists()).toBe(false)
  expect(wrapper.find('.make-pet').exists()).toBe(true)
  wrapper.unmount()
})

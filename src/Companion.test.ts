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
  await navigate('Companions')
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
  await wrapper.findAll('nav button').find(button => button.text() === 'Assistant')!.trigger('click')
  await Promise.resolve()
  expect(main.scrollTop).toBe(0)
  expect(wrapper.findAll('nav button').find(button => button.text() === 'Assistant')!.attributes('aria-current')).toBe('page')
  wrapper.unmount()
})

it('combines companion name search and locomotion filters', async () => {
  snapshot.value = {
    settings: { paused: false, hidden: false, speedScale: 1 }, notes: [], packs: [], perchCount: 0,
    pets: [
      { id: 1, packId: 'pip', name: 'Pip', class: 'flier', state: 'rest', energy: 1, mood: .7, surface: null },
      { id: 2, packId: 'glass', name: 'Glass', class: 'climber', state: 'rest', energy: 1, mood: .7, surface: null },
    ],
  } as unknown as Snapshot
  const wrapper = mount(Companion)
  await wrapper.get('[aria-label="Find a companion"]').setValue('PIP')
  expect(wrapper.findAll('.pet-info')).toHaveLength(1)
  expect(wrapper.get('.pet-info').text()).toContain('Pip')
  await wrapper.get('[aria-label="Find a companion filter"]').setValue('climber')
  expect(wrapper.text()).toContain('No companions match')
  await wrapper.get('[aria-label="Clear search and filters"]').trigger('click')
  expect(wrapper.findAll('.pet-info')).toHaveLength(2)
  wrapper.unmount()
  snapshot.value = null
})

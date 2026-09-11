// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'
import SearchSelect from './SearchSelect.vue'

const mountSelect = (modelValue = '') => mount(SearchSelect, {
  props: { modelValue, options: ['gpt-4.2', 'gpt-5-mini', 'o4-reasoning'], placeholder: 'Pick a model' },
})

it('filters options case-insensitively as the user types', async () => {
  const wrapper = mountSelect()
  await wrapper.get('input').trigger('focus')
  await wrapper.get('input').setValue('MINI')
  expect(wrapper.findAll('li')).toHaveLength(1)
  expect(wrapper.get('li').text()).toBe('gpt-5-mini')
  wrapper.unmount()
})

it('commits the highlighted option with the keyboard', async () => {
  const wrapper = mountSelect()
  const input = wrapper.get('input')
  await input.trigger('focus')
  await input.trigger('keydown', { key: 'ArrowDown' })
  await input.trigger('keydown', { key: 'ArrowDown' })
  await input.trigger('keydown', { key: 'Enter' })
  expect(wrapper.emitted('update:modelValue')).toEqual([['gpt-5-mini']])
  expect(wrapper.find('ul').exists()).toBe(false)
  wrapper.unmount()
})

it('commits unmatched free text so custom model ids stay possible', async () => {
  const wrapper = mountSelect()
  const input = wrapper.get('input')
  await input.trigger('focus')
  await input.setValue('not-a-listed-model')
  await input.trigger('keydown', { key: 'Enter' })
  expect(wrapper.emitted('update:modelValue')).toEqual([['not-a-listed-model']])
  wrapper.unmount()
})

it('discards typed text on blur instead of committing it', async () => {
  const wrapper = mountSelect('gpt-4.2')
  const input = wrapper.get('input')
  await input.trigger('focus')
  await input.setValue('gpt')
  await input.trigger('blur')
  expect((input.element as HTMLInputElement).value).toBe('gpt-4.2')
  expect(wrapper.emitted('update:modelValue')).toBeUndefined()
  wrapper.unmount()
})
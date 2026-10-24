// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import MarkdownResult from './MarkdownResult.vue'

it('renders prose, lists, code and links and routes deliberate clicks to the app', async () => {
  const wrapper = mount(MarkdownResult, { props: { text: '**Hello**\n\n1. Read [the guide](https://example.com/guide)\n2. Try `code`' } })
  await vi.waitFor(() => expect(wrapper.find('strong').exists()).toBe(true))
  expect(wrapper.get('strong').text()).toBe('Hello')
  expect(wrapper.findAll('ol li')).toHaveLength(2)
  expect(wrapper.get('code').text()).toBe('code')
  await wrapper.get('a').trigger('click')
  expect(wrapper.emitted('open')).toEqual([['https://example.com/guide']])
})

it('removes active HTML, external images, styles and unsafe link schemes', async () => {
  const wrapper = mount(MarkdownResult, { props: { text: '<script>alert(1)</script><img src="https://example.com/track" onerror="alert(1)"><iframe src="https://example.com"></iframe><p style="position:fixed" onclick="alert(1)">Safe</p><a href="javascript:alert(1)">Bad</a><a href="file:///secret">Local</a>' } })
  await vi.waitFor(() => expect(wrapper.find('p').text()).toBe('Safe'))
  expect(wrapper.find('script, img, iframe, [style], [onclick], [onerror]').exists()).toBe(false)
  expect(wrapper.findAll('a[href]')).toHaveLength(0)
  expect(wrapper.text()).toContain('Safe')
})

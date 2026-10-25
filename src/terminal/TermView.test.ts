// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import TermView from './TermView.vue'
import { terms, type Term } from './terminals'

function fake(id: number): Term {
  const element = document.createElement('div')
  element.dataset.term = String(id)
  return { id, kind: 'shell', worktree: '/w', title: `T${id}`, alive: true, exitCode: null, opened: true, element,
    term: { open() {}, focus() {} }, fit: { fit() {} } } as unknown as Term
}
class FakeObserver { observe() {} unobserve() {} disconnect() {} }
globalThis.ResizeObserver = FakeObserver as unknown as typeof ResizeObserver
afterEach(() => terms.clear())

it('shows only the terminal it is pointed at when its id changes', async () => {
  terms.set(1, fake(1))
  terms.set(2, fake(2))
  const wrapper = mount(TermView, { props: { id: 1 }, attachTo: document.body })
  await wrapper.vm.$nextTick()
  expect(wrapper.element.querySelector('[data-term="1"]')).not.toBeNull()
  await wrapper.setProps({ id: 2 })
  expect(wrapper.element.querySelector('[data-term="2"]')).not.toBeNull()
  expect(wrapper.element.querySelector('[data-term="1"]'), 'the previous terminal must not stay underneath').toBeNull()
  wrapper.unmount()
})

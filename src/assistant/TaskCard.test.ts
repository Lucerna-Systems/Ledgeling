// @vitest-environment jsdom
import { mount, flushPromises } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import TaskCard from './TaskCard.vue'
import type { Task } from './types'
const api = vi.hoisted(() => ({ call: vi.fn(), refresh: vi.fn() }))
vi.mock('./state', () => ({ taskCall: api.call, refreshTasks: api.refresh }))
const makeTask = (status = 'waiting'): Task => ({ id: 1, request: { prompt: 'Organise files', petId: 2, mode: 'standard', preset: 'sort_type', roots: [], attachments: [], parentId: null, allowCloud: false, allowWeb: false }, connection: { kind: 'none', model: '', endpoint: '', executable: '' }, status, progress: 'Review changes', result: '', error: null, createdAt: 0, operations: [], sources: [], approval: { id: 42, title: 'Review file changes', details: { operations: [{ kind: 'move', source: '/chosen/a.txt', destination: '/chosen/Documents/a.txt', content: null, state: 'planned' }] }, summary: { headline: 'Move 1 file into Documents', items: ['a.txt → Documents/a.txt'], more: 0, note: 'Nothing existing is overwritten.', accept: 'Approve', monospace: false } } })
describe('task approvals', () => {
  beforeEach(() => { api.call.mockReset().mockResolvedValue(undefined); api.refresh.mockReset().mockResolvedValue(undefined) })
  it('does not execute on render and sends the exact approval identifier', async () => {
    const wrapper = mount(TaskCard, { props: { task: makeTask() } })
    expect(api.call).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('/chosen/Documents/a.txt')
    await wrapper.findAll('button').find(b => b.text() === 'Approve')!.trigger('click')
    await flushPromises()
    expect(api.call).toHaveBeenCalledWith('task_approve', { id: 1, approvalId: 42, accept: true })
  })
  it('leads with the plain-words summary and never renders the runtime payload', async () => {
    const task = makeTask()
    task.approval!.title = 'Review agent command'
    task.approval!.details = { request: { threadId: 'th_01', item: { command: ['rm', '-rf', 'build'] } } }
    task.approval!.summary = { headline: 'Run a command', items: ['rm -rf build'], more: 3, note: 'Commands cannot always be undone.', accept: 'Allow', monospace: true }
    const wrapper = mount(TaskCard, { props: { task } })
    const text = wrapper.text()
    expect(text).toContain('Run a command')
    expect(text).toContain('rm -rf build')
    expect(text).toContain('+3 more')
    expect(text).toContain('Commands cannot always be undone.')
    expect(text).not.toContain('threadId')
    expect(text).not.toContain('th_01')
    expect(wrapper.find('.approval-items.mono').exists()).toBe(true)
    // The accept button says what it does, and still sends the same answer.
    await wrapper.findAll('button').find(b => b.text() === 'Allow')!.trigger('click')
    await flushPromises()
    expect(api.call).toHaveBeenCalledWith('task_approve', { id: 1, approvalId: 42, accept: true })
  })

  it('rejects without calling apply and removes stale approvals after cancellation', async () => {
    const wrapper = mount(TaskCard, { props: { task: makeTask() } })
    await wrapper.findAll('button').find(b => b.text() === 'Reject')!.trigger('click')
    await flushPromises()
    expect(api.call).toHaveBeenCalledWith('task_approve', { id: 1, approvalId: 42, accept: false })
    await wrapper.setProps({ task: makeTask('cancelled') })
    expect(wrapper.find('[aria-label="Pending approval"]').exists()).toBe(false)
  })
  it('shows progress and permits undo only after work stops', async () => {
    const task = makeTask('running'); task.approval = null; task.operations = [{ kind: 'write', source: null, destination: '/chosen/result.txt', content: 'hello', state: 'applied' }]
    const wrapper = mount(TaskCard, { props: { task } })
    expect(wrapper.find('[role="status"]').text()).toBe('Review changes')
    expect(wrapper.text()).not.toContain('Undo file changes')
    await wrapper.setProps({ task: { ...task, status: 'completed' } })
    await wrapper.findAll('button').find(b => b.text() === 'Undo file changes')!.trigger('click')
    expect(api.call).toHaveBeenCalledWith('task_undo', { id: 1 })
  })
})

it('keeps long responses compact until expanded and renders Markdown on expansion', async () => {
  const task = makeTask('completed')
  task.approval = null
  task.result = '**A useful response**\n\n' + 'Readable explanation. '.repeat(40)
  const wrapper = mount(TaskCard, { props: { task } })
  expect(wrapper.find('.markdown-body').exists()).toBe(false)
  await wrapper.findAll('button').find(button => button.text() === 'Read full response')!.trigger('click')
  expect(wrapper.get('.markdown-body strong').text()).toBe('A useful response')
  await wrapper.findAll('button').find(button => button.text() === 'Collapse response')!.trigger('click')
  expect(wrapper.find('.markdown-body').exists()).toBe(false)
})

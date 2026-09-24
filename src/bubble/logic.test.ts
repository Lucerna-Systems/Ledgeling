import { describe, expect, it } from 'vitest'
import { blocker, bubblePath, shortPath, startEvent, taskView, type Draft } from './logic'
import type { Task } from '../assistant/types'

const draft = (over: Partial<Draft> = {}): Draft => ({ mode: 'actions', action: 0, prompt: '', allowCloud: false, allowWeb: false, ...over })
const offline = { runtime: 'none', remote: false, folder: '' }

describe('what a mode still needs', () => {
  it('asking needs a connection and a question', () => {
    expect(blocker(offline, draft({ mode: 'ask', prompt: 'hi' }))).toMatch(/Connect a model/)
    expect(blocker({ ...offline, runtime: 'ollama' }, draft({ mode: 'ask', prompt: '  ' }))).toMatch(/Type a question/)
    expect(blocker({ ...offline, runtime: 'ollama' }, draft({ mode: 'ask', prompt: 'why?' }))).toBeNull()
  })
  it('offline built-in actions need a folder, and free-form text needs a model', () => {
    expect(blocker(offline, draft())).toMatch(/Choose a folder/)
    expect(blocker({ ...offline, folder: '/tmp/x' }, draft())).toBeNull()
    expect(blocker({ ...offline, folder: '/tmp/x' }, draft({ action: 5, prompt: 'tidy' }))).toMatch(/connect a model/)
  })
  it('a connected model takes over actions, and only a remote one needs the content box', () => {
    const local = { runtime: 'ollama', remote: false, folder: '' }
    expect(blocker(local, draft())).toBeNull()
    expect(blocker(local, draft({ action: 5 }))).toMatch(/Describe what to do/)
    const remote = { runtime: 'model', remote: true, folder: '' }
    expect(blocker(remote, draft())).toMatch(/Tick the box/)
    expect(blocker(remote, draft({ allowCloud: true }))).toBeNull()
    // An agent's start approval is its consent.
    expect(blocker({ runtime: 'codex', remote: false, folder: '' }, draft())).toBeNull()
  })
})

describe('the start event', () => {
  it('sends the preset for actions and none for a question', () => {
    expect(startEvent(3, draft({ action: 1, prompt: 'x', allowWeb: true }), 9)).toEqual({
      type: 'start', petId: 3, prompt: 'x', preset: 'sort_date', mode: 'actions', allowCloud: false, allowWeb: false, parentId: 9,
    })
    expect(startEvent(3, draft({ mode: 'ask', action: 1, prompt: 'why?', allowWeb: true }), null)).toMatchObject({
      preset: '', mode: 'ask', allowCloud: true, allowWeb: true, parentId: null,
    })
    expect(startEvent(3, draft({ action: 5, prompt: 'tidy' }), null).preset).toBe('')
  })
})

const task = (over: Partial<Task>): Task => ({
  id: 7, request: { prompt: 'p', petId: 3, roots: [], attachments: [], preset: '', mode: 'standard', parentId: null, allowCloud: false, allowWeb: false },
  connection: { kind: 'none', model: '', endpoint: '', executable: '' }, status: 'running', progress: 'Working', result: '', error: null,
  approval: null, operations: [], sources: [], createdAt: 0, ...over,
})

describe('what the bubble shows of a task', () => {
  it('asks for an approval only while waiting on one', () => {
    const approval = { id: 4, title: 'Review', details: {}, summary: { headline: 'Move 2 files', items: ['a', 'b'], more: 0, note: '', accept: 'Approve', monospace: false } }
    expect(taskView(task({ status: 'waiting', approval })).approval?.id).toBe(4)
    expect(taskView(task({ status: 'cancelled', approval })).approval).toBeNull()
  })
  it('offers undo only once work has stopped with changes applied', () => {
    const operations = [{ kind: 'move', source: '/a', destination: '/b', content: null, state: 'applied' }]
    expect(taskView(task({ status: 'running', operations })).undoable).toBe(false)
    expect(taskView(task({ status: 'completed', operations })).undoable).toBe(true)
    expect(taskView(task({ status: 'undone', operations: [{ ...operations[0], state: 'undone' }] })).undoable).toBe(false)
  })
})

describe('the bubble outline', () => {
  it('points the tail where it is told, clear of the corners', () => {
    expect(bubblePath(370, 300, 100, false)).toContain('L100 300')
    expect(bubblePath(370, 300, 100, true)).toContain('L100 0')
    // Clamped: a pet at the far edge still gets a tail on a straight edge.
    expect(bubblePath(370, 300, -50, false)).toContain('L38 300')
    expect(bubblePath(370, 300, 9999, false)).toContain('L332 300')
  })
})

it('shortens long folder paths to their last two parts', () => {
  expect(shortPath('/Users/me/Documents/Taxes')).toBe('…/Documents/Taxes')
  expect(shortPath('C:\\Users\\me\\Photos')).toBe('…\\me\\Photos')
  expect(shortPath('/tmp')).toBe('/tmp')
})

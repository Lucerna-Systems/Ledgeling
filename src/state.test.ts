// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Snapshot } from './types'

const api = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: api.listen }))

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
const snapshot = (paused: boolean) => ({ settings: { paused } }) as Snapshot

describe('companion feedback', () => {
  beforeEach(() => {
    vi.resetModules()
    api.invoke.mockReset()
    api.listen.mockReset().mockResolvedValue(() => {})
    Object.defineProperty(document, 'hidden', { configurable: true, value: false })
  })

  it('shows an action response without waiting for another snapshot request', async () => {
    const state = await import('./state')
    const result = snapshot(true)
    api.invoke.mockResolvedValue(result)
    await state.action('pause')
    expect(state.snapshot.value).toEqual(result)
    expect(api.invoke.mock.calls).toEqual([['pet_action', { kind: 'pause' }]])
    expect(state.busy.value).toBe(false)
  })

  it('ignores a stale poll that completes after an action', async () => {
    const state = await import('./state')
    const oldPoll = deferred<Snapshot>()
    api.invoke.mockImplementation((command: string) =>
      command === 'get_snapshot' ? oldPoll.promise : Promise.resolve(snapshot(true)))
    const poll = state.refresh()
    await state.action('pause')
    oldPoll.resolve(snapshot(false))
    await poll
    expect(state.settings.value?.paused).toBe(true)
  })

  it('keeps controls busy until all outstanding commands finish', async () => {
    const state = await import('./state')
    const first = deferred<Snapshot>()
    const second = deferred<Snapshot>()
    api.invoke.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const a = state.call('first', {}, false)
    const b = state.call('second', {}, false)
    first.resolve(snapshot(false))
    await a
    expect(state.busy.value).toBe(true)
    second.resolve(snapshot(true))
    await b
    expect(state.busy.value).toBe(false)
  })

  it('refreshes on return to the window and removes listeners on disposal', async () => {
    const state = await import('./state')
    api.invoke.mockResolvedValue(snapshot(false))
    const stop = state.start()
    await Promise.resolve()
    api.invoke.mockClear()
    window.dispatchEvent(new Event('focus'))
    await Promise.resolve()
    expect(api.invoke).toHaveBeenCalledWith('get_snapshot')
    stop()
    api.invoke.mockClear()
    window.dispatchEvent(new Event('focus'))
    document.dispatchEvent(new Event('visibilitychange'))
    expect(api.invoke).not.toHaveBeenCalled()
  })

  it('releases controls and reports failed actions', async () => {
    const state = await import('./state')
    state.snapshot.value = snapshot(false)
    api.invoke.mockRejectedValue(new Error('Could not pause'))
    await state.action('pause')
    expect(state.busy.value).toBe(false)
    expect(state.error.value).toContain('Could not pause')
    expect(state.settings.value?.paused).toBe(false)
  })
})

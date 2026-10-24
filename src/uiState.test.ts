// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { startPersist } from './uiState'
import { restoreDraft, serializeDraft } from './studio/state'
import { starterDraft } from './studio/model'

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => false, invoke: vi.fn(() => Promise.resolve()) }))
afterEach(() => { vi.useRealTimers(); vi.mocked(invoke).mockClear() })

it('round-trips a Studio draft through JSON, pixels included', () => {
  const draft = starterDraft()
  const clip = Object.values(draft.clips)[0]
  clip.frames[0][3] = 255
  clip.frames[0][7] = 17
  draft.name = 'Round trip'
  const restored = restoreDraft(JSON.parse(JSON.stringify(serializeDraft(draft))))!
  expect(restored.name).toBe('Round trip')
  const back = Object.values(restored.clips)[0].frames[0]
  expect(back[3]).toBe(255)
  expect(back[7]).toBe(17)
  expect(back).toBeInstanceOf(Uint8ClampedArray)
  expect(restored.states.map(s => s.key)).toEqual(draft.states.map(s => s.key))
})

it('rejects a malformed saved draft instead of throwing', () => {
  expect(restoreDraft(null)).toBeNull()
  expect(restoreDraft({ name: 'x' })).toBeNull()
  const bad = JSON.parse(JSON.stringify(serializeDraft(starterDraft())))
  Object.values<any>(bad.clips)[0].frames[0] = 'not-base64!'
  expect(restoreDraft(bad)).toBeNull()
})

it('saves UI state once after a burst of changes, and flushes on stop', () => {
  vi.useFakeTimers()
  const page = ref('assistant')
  const stop = startPersist([page], () => ({ page: page.value, prompt: 'hi', studio: null, volatile: false }), 400)
  page.value = 'pets'
  return Promise.resolve().then(() => {
    page.value = 'history'
    return Promise.resolve()
  }).then(() => {
    vi.advanceTimersByTime(400)
    expect(invoke).toHaveBeenCalledTimes(1)
    const json = JSON.parse((vi.mocked(invoke).mock.calls[0][1] as { json: string }).json)
    expect(json).toMatchObject({ page: 'history', prompt: 'hi', volatile: false })
    stop()
  })
})

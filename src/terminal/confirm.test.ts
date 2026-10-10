import { describe, expect, it } from 'vitest'
import { ask, question } from './confirm'

describe('in-app confirmation', () => {
  it('resolves with the answer and clears the question', async () => {
    const answer = ask('Stop Shell?', 'Stop Shell')
    expect(question.value?.action).toBe('Stop Shell')
    question.value!.answer(true)
    expect(await answer).toBe(true)
    expect(question.value).toBeNull()
  })

  it('a newer question answers the older one with no', async () => {
    const first = ask('Delete a?', 'Delete')
    const second = ask('Delete b?', 'Delete')
    expect(await first).toBe(false)
    question.value!.answer(true)
    expect(await second).toBe(true)
  })
})

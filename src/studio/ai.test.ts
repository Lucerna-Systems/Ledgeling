import { describe, expect, it } from 'vitest'
import { apply, behaviourPrompt, parseSuggestion, restore, snapshot } from './ai'
import { buildManifest, problems, starterDraft } from './model'

const reply = (over: object = {}) => JSON.stringify({
  initial: 'rest',
  states: [
    { name: 'rest', clip: 'idle', action: '', duration: [5, 15], next: [{ to: 'hide', weight: 2, when: [{ signal: 'app_category', op: '==', value: 'game' }] }, { to: 'rest', weight: 1 }] },
    { name: 'hide', clip: 'sleep', action: 'flee_from_cursor', duration: null, next: [{ to: 'rest', weight: 1 }] },
  ],
  reactions: [{ on: 'clicked', goto: 'rest', cooldown: 3 }],
  ...over,
})

describe('studio AI suggestions', () => {
  it('reads a clean reply into a design the validator accepts', () => {
    const draft = starterDraft()
    const s = parseSuggestion(reply(), draft)
    expect(s.dropped).toBe(0)
    apply(draft, s)
    expect(draft.states.map(x => x.name)).toEqual(['rest', 'hide'])
    expect((buildManifest(draft).behavior.states as Record<string, { transitions: unknown[] }>).rest!.transitions[0]).toMatchObject({ to: 'hide', when: 'app_category == game' })
    expect(problems(draft).filter(p => !p.startsWith('Draw'))).toEqual([])
  })

  it('tolerates a code fence and chatter around the JSON', () => {
    expect(parseSuggestion(`Sure!\n\`\`\`json\n${reply()}\n\`\`\`\nEnjoy.`, starterDraft()).states).toHaveLength(2)
  })

  it('drops anything outside the closed lists instead of trusting it', () => {
    const bad = reply({
      states: [{
        name: 'Rest', clip: 'fly_to_moon', action: 'run_shell', duration: [9, 1],
        next: [
          { to: 'Rest', weight: -4, when: [{ signal: 'window_title', op: '==', value: 'x' }, { signal: 'energy', op: '>', value: 'lots' }, { signal: 'energy', op: '>', value: '0.5' }] },
          { to: 'nowhere', weight: 1 },
        ],
      }],
      reactions: [{ on: 'exec', goto: 'rest' }, { on: 'clicked', goto: 'nowhere' }],
    })
    const s = parseSuggestion(bad, starterDraft())
    const [rest] = s.states
    expect(rest).toMatchObject({ name: 'rest', clip: 'idle', action: '', duration: null })
    expect(rest!.transitions).toEqual([{ to: 'rest', weight: 1, when: [{ signal: 'energy', op: '>', value: '0.5' }] }])
    expect(s.reactions).toEqual([])
    expect(s.dropped).toBeGreaterThan(5)
  })

  it('only offers animations this kind of pet can use', () => {
    const s = parseSuggestion(reply({ states: [{ name: 'rest', clip: 'climb', next: [] }] }), starterDraft('walker'))
    expect(s.states[0]!.clip).toBe('idle')
  })

  it('never leaves a behaviour with nowhere to go, and falls back to the first as the start', () => {
    const s = parseSuggestion(reply({ initial: 'ghost', states: [{ name: 'a' }, { name: 'b' }] }), starterDraft())
    expect(s.initial).toBe('a')
    expect(s.states.every(x => x.transitions.length === 1 && x.transitions[0]!.to === 'a')).toBe(true)
  })

  it('refuses replies that are not a design', () => {
    expect(() => parseSuggestion('I cannot do that.', starterDraft())).toThrow(/did not contain/)
    expect(() => parseSuggestion('{"states": []}', starterDraft())).toThrow(/no usable/)
    expect(() => parseSuggestion('{not json}', starterDraft())).toThrow(/readable/)
  })

  it('undo puts the earlier behaviours back', () => {
    const draft = starterDraft()
    const before = snapshot(draft)
    apply(draft, parseSuggestion(reply(), draft))
    restore(draft, before)
    expect(draft.states.map(s => s.name)).toEqual(['rest', 'wander', 'nap', 'say_hi'])
  })

  it('sends the request and outline but no pixels', () => {
    const draft = starterDraft()
    draft.clips.idle!.frames[0]![3] = 255
    const prompt = behaviourPrompt(draft, 'be shy')
    expect(prompt).toContain('be shy')
    expect(prompt).toContain('"name":"wander"')
    expect(prompt).not.toMatch(/255,0,0|Uint8/)
  })
})

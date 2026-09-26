import { describe, expect, it } from 'vitest'
import { FRAME, blankFrame, buildManifest, problems, renameState, starterDraft, strip } from './model'

function drawnFrame() {
  const frame = blankFrame()
  frame.set([255, 0, 0, 255], (10 * FRAME + 10) * 4)
  return frame
}
function ready() {
  const draft = starterDraft('climber')
  draft.name = 'Pebble the Newt'
  draft.clips.idle!.frames = [drawnFrame(), drawnFrame()]
  draft.clips.move!.frames = [drawnFrame()]
  return draft
}

describe('studio manifest', () => {
  it('writes a pack the format understands, with only drawn animations', () => {
    const draft = ready()
    draft.clips.jump = { frames: [blankFrame()], fps: 10, loop: false }
    const manifest = buildManifest(draft)
    expect(manifest.id).toBe('com.studio.pebble-the-newt')
    expect(manifest.locomotion.class).toBe('climber')
    expect(manifest.sprite.surface_orientation).toBe('align')
    expect(Object.keys(manifest.animations)).toEqual(['idle', 'move'])
    expect(manifest.animations.idle).toEqual({ file: 'sprites/idle.png', frames: 2, fps: 8 })
    expect(manifest.behavior.initial).toBe('rest')
  })

  it('joins block conditions into the fixed grammar and leaves unconditional steps bare', () => {
    const draft = ready()
    draft.states[0]!.transitions[1]!.when.push({ signal: 'app_category', op: '!=', value: 'game' })
    const rest = buildManifest(draft).behavior.states.rest as { transitions: { when?: string }[] }
    expect(rest.transitions[0]).not.toHaveProperty('when')
    expect(rest.transitions[1]!.when).toBe('idle_seconds > 240 && app_category != game')
  })

  it('keeps optional fields out unless they were set', () => {
    const draft = ready()
    const manifest = buildManifest(draft)
    const wander = manifest.behavior.states.wander as Record<string, unknown>
    expect(wander.action).toBe('move_to_random_surface')
    expect(wander).not.toHaveProperty('duration')
    expect(manifest.reactions.find(r => r.on === 'user_idle_long')).toEqual({ on: 'user_idle_long', goto: 'nap' })
  })
})

describe('studio checks', () => {
  it('asks for the two required animations and a name', () => {
    const draft = starterDraft()
    draft.name = ' '
    expect(problems(draft)).toEqual([
      'Give your pet a name.',
      'Draw at least one frame of the Idle animation.',
      'Draw at least one frame of the Move animation.',
    ])
    expect(problems(ready())).toEqual([])
  })

  it('saves a blank Move as a copy of Idle until issue #11 is fixed', () => {
    const draft = ready()
    draft.clips.move!.frames = [blankFrame()]
    expect(problems(draft)).toEqual([])
    expect(buildManifest(draft).animations.move).toEqual({ file: 'sprites/move.png', frames: 2, fps: 8 })
  })

  it('catches a condition without a number and a step to a missing behaviour', () => {
    const draft = ready()
    draft.states[0]!.transitions[1]!.when[0]!.value = 'soon'
    draft.states[1]!.transitions[0]!.to = 'gone'
    expect(problems(draft)).toEqual([
      'A condition in “rest” needs a number.',
      '“wander” goes to a behaviour that no longer exists.',
    ])
  })

  it('renaming a behaviour follows every reference to it', () => {
    const draft = ready()
    renameState(draft, 'rest', 'lounge')
    expect(draft.initial).toBe('lounge')
    expect(draft.states[1]!.transitions[0]!.to).toBe('lounge')
    expect(problems(draft)).toEqual([])
  })
})

it('lays frames side by side in one strip', () => {
  const a = blankFrame(); a.set([1, 2, 3, 255], 0)
  const b = blankFrame(); b.set([4, 5, 6, 255], 0)
  const s = strip([a, b])
  expect([s.width, s.height]).toEqual([FRAME * 2, FRAME])
  expect(Array.from(s.pixels.subarray(0, 4))).toEqual([1, 2, 3, 255])
  expect(Array.from(s.pixels.subarray(FRAME * 4, FRAME * 4 + 4))).toEqual([4, 5, 6, 255])
})

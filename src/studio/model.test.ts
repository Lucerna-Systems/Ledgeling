import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { FRAME, blankFrame, buildManifest, draftFromPack, emptyClip, problems, renameState, starterDraft, strip } from './model'

function drawnFrame() {
  const frame = blankFrame()
  frame.set([255, 0, 0, 255], (10 * FRAME + 10) * 4)
  return frame
}
function ready() {
  const draft = starterDraft('climber')
  draft.name = 'Pebble the Newt'
  draft.clips.idle!.frames = [drawnFrame(), drawnFrame()]
  draft.clips.move = { ...emptyClip(), frames: [drawnFrame()] }
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
  it('asks for a name and at least one drawn animation', () => {
    const draft = starterDraft()
    draft.name = ' '
    expect(problems(draft)).toEqual([
      'Give your pet a name.',
      'Draw at least one frame of any animation, such as Idle.',
    ])
    expect(problems(ready())).toEqual([])
  })

  it('lets a pet have only an Idle animation', () => {
    const draft = ready()
    delete draft.clips.move
    expect(problems(draft)).toEqual([])
    expect(Object.keys(buildManifest(draft).animations)).toEqual(['idle'])
  })

  it('does not write an undrawn Move', () => {
    const draft = ready()
    draft.clips.move!.frames = [blankFrame()]
    expect(Object.keys(buildManifest(draft).animations)).toEqual(['idle'])
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

describe('opening an installed pack', () => {
  const root = new URL('../../packs/squirrel-red/pet.json', import.meta.url)
  const manifest = JSON.parse(readFileSync(root, 'utf8'))
  const opened = () => ({
    manifest: JSON.stringify(manifest),
    strips: Object.entries<{ frames: number }>(manifest.animations).map(([name, a]) => {
      const width = FRAME * a.frames
      const pixels = new Array(width * FRAME * 4).fill(0)
      pixels[3] = 255
      return { name, width, height: FRAME, pixels }
    }),
  })

  it('reproduces the bundled squirrel\'s states, reactions and frame counts', () => {
    const draft = draftFromPack(opened())
    const rebuilt = buildManifest(draft)
    expect(Object.keys(rebuilt.behavior.states)).toEqual(Object.keys(manifest.behavior.states))
    expect(rebuilt.behavior.states.rest).toEqual(manifest.behavior.states.rest)
    expect(rebuilt.reactions.map(r => r.on)).toEqual(manifest.reactions.map((r: { on: string }) => r.on))
    for (const [name, a] of Object.entries<{ frames: number; fps: number }>(manifest.animations)) {
      expect(draft.clips[name]!.frames).toHaveLength(a.frames)
      expect(rebuilt.animations[name]).toMatchObject({ frames: a.frames, fps: a.fps })
    }
    expect(draft.id).toBe(manifest.id)
  })

  it('makes a template copy with a new id and name', () => {
    const draft = draftFromPack(opened(), { copy: true })
    expect(draft.name).toBe(`Copy of ${manifest.name}`)
    const rebuilt = buildManifest(draft)
    expect(rebuilt.id).not.toBe(manifest.id)
    expect(rebuilt.id.startsWith('com.studio.copy-of-')).toBe(true)
  })

  it('keeps a condition the blocks cannot show, and starts a pack without behavior from the starter', () => {
    const odd = JSON.parse(opened().manifest)
    odd.behavior.states.rest.transitions[0].when = 'hour >= 22 || hour < 6'
    const draft = draftFromPack({ ...opened(), manifest: JSON.stringify(odd) })
    const rest = buildManifest(draft).behavior.states.rest as { transitions: { when?: string }[] }
    expect(rest.transitions[0]!.when).toBe('hour >= 22 || hour < 6')

    delete odd.behavior
    delete odd.reactions
    const plain = draftFromPack({ ...opened(), manifest: JSON.stringify(odd) })
    expect(plain.initial).toBe('rest')
    expect(plain.states.map(s => s.name)).toContain('wander')
  })
})

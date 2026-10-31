import { describe, expect, it } from 'vitest'
import { decorFromOpened, decorManifest, decorPayload, decorProblems, newProp, resize, slug, starterDecor, type OpenedDecor } from './decorModel'

function painted(width = 4, height = 4) {
  const prop = newProp([], width, height)
  prop.pixels.fill(255)
  return prop
}

describe('decor drafts', () => {
  it('names new props uniquely and starts with nothing drawn', () => {
    const first = newProp([])
    expect(first.name).toBe('prop')
    expect(newProp(['prop']).name).not.toBe('prop')
    expect(first.pixels.every(v => v === 0)).toBe(true)
  })

  it('lists what stops a save in plain words', () => {
    const draft = starterDecor()
    expect(decorProblems(draft)).toEqual(['Give the decor pack a name.', 'Draw something for “prop”.'])
    draft.name = 'Garden'
    draft.props = [painted(), { ...painted(), name: 'Bad Name' }, painted()]
    draft.props[2].perch = { y: 0.2, from: 0.8, to: 0.1 }
    const problems = decorProblems(draft)
    expect(problems.some(p => p.includes('lowercase'))).toBe(true)
    expect(problems.some(p => p.includes('Two props are called'))).toBe(true)
    expect(problems.some(p => p.includes('left to right'))).toBe(true)
  })

  it('resizes from the base so a prop keeps standing on the edge', () => {
    const prop = painted(2, 2)
    const bigger = resize(prop, 4, 4)
    expect([bigger.width, bigger.height]).toEqual([4, 4])
    const alpha = (x: number, y: number) => bigger.pixels[(y * 4 + x) * 4 + 3]
    expect(alpha(1, 3)).toBe(255)
    expect(alpha(1, 0)).toBe(0)
    expect(resize(prop, 500, 0).width).toBe(96)
    expect(resize(prop, 500, 0).height).toBe(4)
  })

  it('writes a manifest and sprites the Rust loader accepts', () => {
    const draft = { name: 'My Garden!', author: '', props: [{ ...painted(3, 5), name: 'tree', sway: 'tall' as const, perch: { y: 0.1, from: 0.2, to: 0.8 } }] }
    expect(slug(draft.name)).toBe('my-garden')
    const manifest = decorManifest(draft)
    expect(manifest.id).toBe('com.studio.decor-my-garden')
    expect(manifest.props).toEqual({ tree: { file: 'sprites/tree.png', width: 3, height: 5, sway: 'tall', perch: { y: 0.1, from: 0.2, to: 0.8 } } })
    const payload = decorPayload(draft)
    expect(JSON.parse(payload.manifest).author).toBe('Me')
    expect(payload.sprites[0]).toMatchObject({ name: 'tree', width: 3, height: 5 })
    expect(payload.sprites[0].pixels).toHaveLength(3 * 5 * 4)
  })

  it('writes animations only for props that have some', () => {
    const still = { ...painted(), name: 'rock' }
    const pinwheel = { ...painted(), name: 'pinwheel', animations: ['spin' as const, 'rustle' as const] }
    const manifest = decorManifest({ name: 'Yard', author: '', props: [still, pinwheel] })
    expect(manifest.props.rock).not.toHaveProperty('animations')
    expect(manifest.props.pinwheel).toMatchObject({ sway: 'soft', animations: ['spin', 'rustle'] })
  })

  it('keeps an id it was given, so an in-place edit replaces the pack', () => {
    expect(decorManifest({ id: 'com.someone.garden', name: 'Other', author: '', props: [painted()] }).id).toBe('com.someone.garden')
  })
})

describe('opening a decor pack', () => {
  // Two 2 × 2 frames side by side: red, then blue.
  const strip = Array.from({ length: 2 }, () => [[255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255]].flat()).flat()
  const opened = (): OpenedDecor => ({
    manifest: JSON.stringify({
      format: 1, id: 'com.ana.garden', name: 'Garden', author: 'Ana', version: '1.0.0',
      props: {
        flag: { file: 'sprites/flag.png', width: 2, height: 2, frames: 2, fps: 4, sway: { stiffness: 3 }, animations: ['bob', 'explode', 'bob'] },
        tree: { file: 'sprites/tree.png', width: 3, height: 4, sway: 'tall', perch: { y: 0.25, from: 0.1, to: 0.9 } },
      },
    }),
    sprites: [
      { name: 'flag', width: 4, height: 2, pixels: strip },
      { name: 'tree', width: 3, height: 4, pixels: new Array(3 * 4 * 4).fill(9) },
    ],
  })

  it('opens a copy under a new name and no id', () => {
    const draft = decorFromOpened(opened(), 'copy')
    expect(draft.name).toBe('Garden copy')
    expect(draft.id).toBeUndefined()
    expect(draft.author).toBe('Ana')
    expect(decorManifest(draft).id).toBe('com.studio.decor-garden-copy')
  })

  it('keeps the id for an in-place edit', () => {
    const draft = decorFromOpened(opened(), 'in_place')
    expect(draft).toMatchObject({ id: 'com.ana.garden', name: 'Garden' })
    expect(decorManifest(draft).id).toBe('com.ana.garden')
  })

  it('keeps only the first frame of a strip, maps a sway object to soft and drops unknown animations', () => {
    const [flag, tree] = decorFromOpened(opened(), 'copy').props
    expect(flag).toMatchObject({ name: 'flag', width: 2, height: 2, sway: 'soft', perch: null, animations: ['bob'] })
    expect(Array.from(flag!.pixels)).toEqual([255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255])
    expect(tree).toMatchObject({ sway: 'tall', perch: { y: 0.25, from: 0.1, to: 0.9 }, animations: [] })
    expect(tree!.pixels).toHaveLength(3 * 4 * 4)
  })
})

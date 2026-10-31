import { describe, expect, it } from 'vitest'
import { decorManifest, decorPayload, decorProblems, newProp, resize, slug, starterDecor } from './decorModel'

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
})

import { describe, expect, it } from 'vitest'
import { ids, insert, keep, parse, remove, resize, type Layout } from './layout'

const pane = (id: number): Layout => ({ type: 'pane', id })

describe('terminal layout', () => {
  it('starts with one pane and splits beside the target', () => {
    let layout = insert(null, null, 1, 'row')
    expect(layout).toEqual(pane(1))
    layout = insert(layout, 1, 2, 'row')
    expect(layout).toEqual({ type: 'split', dir: 'row', sizes: [0.5, 0.5], children: [pane(1), pane(2)] })
  })

  it('adds a sibling to a split running the same way instead of nesting', () => {
    let layout = insert(insert(null, null, 1, 'row'), 1, 2, 'row')
    layout = insert(layout, 1, 3, 'row')
    expect(ids(layout)).toEqual([1, 3, 2])
    expect(layout.type === 'split' && layout.sizes).toEqual([0.25, 0.25, 0.5])
  })

  it('nests a split running the other way', () => {
    let layout = insert(insert(null, null, 1, 'row'), 1, 2, 'row')
    layout = insert(layout, 2, 3, 'column')
    expect(layout).toEqual({
      type: 'split', dir: 'row', sizes: [0.5, 0.5],
      children: [pane(1), { type: 'split', dir: 'column', sizes: [0.5, 0.5], children: [pane(2), pane(3)] }],
    })
  })

  it('gives a closed pane its space back and folds one-child splits', () => {
    let layout: Layout | null = insert(insert(null, null, 1, 'row'), 1, 2, 'row')
    layout = insert(layout, 2, 3, 'column')
    layout = remove(layout, 3)
    expect(layout).toEqual({ type: 'split', dir: 'row', sizes: [0.5, 0.5], children: [pane(1), pane(2)] })
    expect(remove(remove(layout, 1), 2)).toBeNull()
  })

  it('keeps only live terminals and renormalizes', () => {
    const layout: Layout = { type: 'split', dir: 'row', sizes: [0.2, 0.3, 0.5], children: [pane(1), pane(2), pane(3)] }
    const kept = keep(layout, id => id !== 3)
    expect(kept).toEqual({ type: 'split', dir: 'row', sizes: [0.4, 0.6], children: [pane(1), pane(2)] })
  })

  it('never drags a pane below the minimum', () => {
    expect(resize([0.5, 0.5], 0, 0.1)).toEqual([0.6, 0.4])
    const squeezed = resize([0.5, 0.5], 0, 0.9)
    expect(squeezed[1]).toBeCloseTo(0.08)
    expect(squeezed[0] + squeezed[1]).toBeCloseTo(1)
  })

  it('rejects malformed stored layouts', () => {
    expect(parse({ type: 'pane', id: 4 })).toEqual(pane(4))
    expect(parse({ type: 'split', dir: 'diagonal', children: [] })).toBeNull()
    expect(parse({ type: 'split', dir: 'row', children: [{ type: 'pane', id: 'x' }] })).toBeNull()
    expect(parse({ type: 'split', dir: 'column', sizes: 'bad', children: [{ type: 'pane', id: 1 }, { type: 'pane', id: 2 }] }))
      .toEqual({ type: 'split', dir: 'column', sizes: [0.5, 0.5], children: [pane(1), pane(2)] })
  })
})

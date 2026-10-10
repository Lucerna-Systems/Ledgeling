/** A worktree's terminals as nested horizontal and vertical splits. Leaves
 *  are backend session ids; `sizes` are fractions that sum to 1. */
export type Layout =
  | { type: 'pane'; id: number }
  | { type: 'split'; dir: Direction; sizes: number[]; children: Layout[] }

/** `row` puts panes side by side, `column` stacks them. */
export type Direction = 'row' | 'column'

/** The smallest share a pane can be dragged down to. */
export const MIN_SIZE = 0.08

export function ids(layout: Layout | null): number[] {
  if (!layout) return []
  return layout.type === 'pane' ? [layout.id] : layout.children.flatMap(ids)
}

/** Add `id` beside `target`, splitting in `dir`. A split already running in
 *  that direction gains a sibling instead of nesting one level deeper. */
export function insert(layout: Layout | null, target: number | null, id: number, dir: Direction): Layout {
  const leaf: Layout = { type: 'pane', id }
  if (!layout) return leaf
  if (target === null || !ids(layout).includes(target)) {
    if (layout.type === 'split' && layout.dir === dir) return append(layout, leaf, layout.children.length - 1)
    return { type: 'split', dir, sizes: [0.5, 0.5], children: [layout, leaf] }
  }
  return place(layout, target, leaf, dir)
}

function append(split: Extract<Layout, { type: 'split' }>, leaf: Layout, after: number): Layout {
  // The new pane takes half of the one it was opened from.
  const sizes = [...split.sizes]
  const half = sizes[after] / 2
  sizes.splice(after, 1, half, half)
  const children = [...split.children]
  children.splice(after + 1, 0, leaf)
  return { ...split, sizes, children }
}

function place(node: Layout, target: number, leaf: Layout, dir: Direction): Layout {
  if (node.type === 'pane') {
    return node.id === target ? { type: 'split', dir, sizes: [0.5, 0.5], children: [node, leaf] } : node
  }
  const at = node.children.findIndex(c => c.type === 'pane' && c.id === target)
  if (at >= 0 && node.dir === dir) return append(node, leaf, at)
  return { ...node, children: node.children.map(c => place(c, target, leaf, dir)) }
}

/** Drop `id`; its space goes to its neighbours and one-child splits fold away. */
export function remove(layout: Layout | null, id: number): Layout | null {
  return keep(layout, other => other !== id)
}

/** Keep only the panes `alive` accepts. */
export function keep(layout: Layout | null, alive: (id: number) => boolean): Layout | null {
  if (!layout) return null
  if (layout.type === 'pane') return alive(layout.id) ? layout : null
  const kept: { child: Layout; size: number }[] = []
  layout.children.forEach((child, i) => {
    const next = keep(child, alive)
    if (next) kept.push({ child: next, size: layout.sizes[i] ?? 0 })
  })
  if (!kept.length) return null
  if (kept.length === 1) return kept[0].child
  const total = kept.reduce((sum, k) => sum + k.size, 0) || 1
  return { ...layout, children: kept.map(k => k.child), sizes: kept.map(k => k.size / total) }
}

/** Move the divider after child `index` by `delta` (a fraction of the split). */
export function resize(sizes: number[], index: number, delta: number): number[] {
  const next = [...sizes]
  const pair = next[index] + next[index + 1]
  const first = Math.min(Math.max(next[index] + delta, MIN_SIZE), pair - MIN_SIZE)
  next[index] = first
  next[index + 1] = pair - first
  return next
}

/** Whatever came back from storage, if it is still a well-formed layout. */
export function parse(value: unknown): Layout | null {
  if (!value || typeof value !== 'object') return null
  const node = value as Record<string, unknown>
  if (node.type === 'pane') return typeof node.id === 'number' ? { type: 'pane', id: node.id } : null
  if (node.type !== 'split' || (node.dir !== 'row' && node.dir !== 'column') || !Array.isArray(node.children)) return null
  const children = node.children.map(parse)
  if (children.some(c => !c)) return null
  const sizes = Array.isArray(node.sizes) && node.sizes.length === children.length && node.sizes.every(s => typeof s === 'number' && s > 0)
    ? node.sizes as number[]
    : children.map(() => 1 / children.length)
  return keep({ type: 'split', dir: node.dir, sizes, children: children as Layout[] }, () => true)
}

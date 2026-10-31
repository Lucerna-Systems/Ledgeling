/** Studio's Decor mode: a decor pack being drawn, and how it becomes the
 *  `decor.json` and sprites Rust validates and saves. */

/** Props are scenery on a window edge, not second pets. */
export const MAX_SIDE = 96
export const SWAYS = [
  { value: 'stiff', label: 'Stiff', blurb: 'Barely moves: rocks, mushrooms' },
  { value: 'soft', label: 'Soft', blurb: 'Wobbles and settles: grass, flowers, bushes' },
  { value: 'tall', label: 'Tall', blurb: 'Leans slowly and swings: trees' },
] as const
export type Sway = typeof SWAYS[number]['value']

export interface Ledge { y: number; from: number; to: number }
export interface DecorProp {
  name: string
  width: number
  height: number
  sway: Sway
  perch: Ledge | null
  /** RGBA, `width × height × 4` bytes. */
  pixels: Uint8ClampedArray
}
export interface DecorDraft { name: string; author: string; props: DecorProp[] }

export const PROP_NAME = /^[a-z_]{1,32}$/

export function newProp(taken: string[], width = 32, height = 32): DecorProp {
  let name = 'prop'
  for (let i = 2; taken.includes(name); i++) name = `prop_${'abcdefghijklmnopqrstuvwxyz'[(i - 2) % 26]}`
  return { name, width, height, sway: 'soft', perch: null, pixels: new Uint8ClampedArray(width * height * 4) }
}

export function starterDecor(): DecorDraft {
  return { name: '', author: '', props: [newProp([])] }
}

const clampSide = (n: number) => Math.min(MAX_SIDE, Math.max(4, Math.round(Number.isFinite(n) ? n : 32)))

/** A new size, keeping what was drawn where it overlaps, anchored at the bottom
 *  middle so a prop keeps standing on its base. */
export function resize(prop: DecorProp, width: number, height: number): DecorProp {
  const w = clampSide(width)
  const h = clampSide(height)
  const pixels = new Uint8ClampedArray(w * h * 4)
  const dx = Math.floor((w - prop.width) / 2)
  const dy = h - prop.height
  for (let y = 0; y < prop.height; y++) {
    for (let x = 0; x < prop.width; x++) {
      const tx = x + dx
      const ty = y + dy
      if (tx < 0 || ty < 0 || tx >= w || ty >= h) continue
      pixels.set(prop.pixels.subarray((y * prop.width + x) * 4, (y * prop.width + x) * 4 + 4), (ty * w + tx) * 4)
    }
  }
  return { ...prop, width: w, height: h, pixels }
}

export function slug(name: string): string {
  return name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40)
}

/** Everything that stops a save, in plain words, most important first. */
export function decorProblems(draft: DecorDraft): string[] {
  const out: string[] = []
  if (!slug(draft.name)) out.push('Give the decor pack a name.')
  if (!draft.props.length) out.push('Add at least one prop.')
  const seen = new Set<string>()
  for (const prop of draft.props) {
    if (!PROP_NAME.test(prop.name)) out.push(`“${prop.name}” needs a name of lowercase letters and underscores.`)
    else if (seen.has(prop.name)) out.push(`Two props are called “${prop.name}”.`)
    seen.add(prop.name)
    if (!prop.pixels.some((v, i) => i % 4 === 3 && v > 0)) out.push(`Draw something for “${prop.name}”.`)
    if (prop.perch && prop.perch.from >= prop.perch.to) out.push(`The ledge on “${prop.name}” has to run left to right.`)
  }
  return out
}

export function decorManifest(draft: DecorDraft) {
  const props: Record<string, unknown> = {}
  for (const p of draft.props) {
    props[p.name] = {
      file: `sprites/${p.name}.png`,
      width: p.width,
      height: p.height,
      sway: p.sway,
      ...(p.perch ? { perch: { ...p.perch } } : {}),
    }
  }
  return {
    format: 1,
    id: `com.studio.decor-${slug(draft.name)}`,
    name: draft.name.trim(),
    author: draft.author.trim() || 'Me',
    version: '1.0.0',
    props,
  }
}

/** The arguments for `save_decor_pack`. */
export function decorPayload(draft: DecorDraft) {
  return {
    manifest: JSON.stringify(decorManifest(draft), null, 2),
    sprites: draft.props.map(p => ({ name: p.name, width: p.width, height: p.height, pixels: Array.from(p.pixels) })),
  }
}

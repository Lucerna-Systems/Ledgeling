/** The Decor Studio: a decor pack being drawn, and how it becomes the
 *  `decor.json` and sprites Rust validates and saves. */

/** Props are scenery on a window edge, not second pets. */
export const MAX_SIDE = 96
export const SWAYS = [
  { value: 'stiff', label: 'Stiff', blurb: 'Barely moves: rocks, mushrooms' },
  { value: 'soft', label: 'Soft', blurb: 'Wobbles and settles: grass, flowers, bushes' },
  { value: 'tall', label: 'Tall', blurb: 'Leans slowly and swings: trees' },
] as const
export type Sway = typeof SWAYS[number]['value']
/** Motion a prop has on its own, on top of how it sways when its window moves. */
export const PROP_ANIMATIONS = [
  { value: 'breeze', label: 'Breeze', blurb: 'Sways gently all the time, as if in wind' },
  { value: 'bob', label: 'Bob', blurb: 'Floats softly up and down' },
  { value: 'spin', label: 'Spin', blurb: 'Turns around its centre, like a pinwheel' },
  { value: 'twinkle', label: 'Twinkle', blurb: 'Glows softly on and off' },
  { value: 'rustle', label: 'Rustle', blurb: 'Shakes when your pointer brushes past' },
] as const
export type PropAnimation = typeof PROP_ANIMATIONS[number]['value']

export interface Ledge { y: number; from: number; to: number }
export interface DecorProp {
  name: string
  width: number
  height: number
  sway: Sway
  perch: Ledge | null
  animations: PropAnimation[]
  /** RGBA, `width × height × 4` bytes. */
  pixels: Uint8ClampedArray
}
export interface DecorDraft {
  /** Set when editing an installed pack in place, so saving replaces it. */
  id?: string
  name: string
  author: string
  props: DecorProp[]
}

export const PROP_NAME = /^[a-z_]{1,32}$/

export function newProp(taken: string[], width = 32, height = 32): DecorProp {
  let name = 'prop'
  for (let i = 2; taken.includes(name); i++) name = `prop_${'abcdefghijklmnopqrstuvwxyz'[(i - 2) % 26]}`
  return { name, width, height, sway: 'soft', perch: null, animations: [], pixels: new Uint8ClampedArray(width * height * 4) }
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

/** The pack id: kept when editing in place, otherwise made from the name. */
export const decorId = (draft: DecorDraft) => draft.id ?? `com.studio.decor-${slug(draft.name)}`

export function decorManifest(draft: DecorDraft) {
  const props: Record<string, unknown> = {}
  for (const p of draft.props) {
    props[p.name] = {
      file: `sprites/${p.name}.png`,
      width: p.width,
      height: p.height,
      sway: p.sway,
      ...(p.perch ? { perch: { ...p.perch } } : {}),
      ...(p.animations.length ? { animations: [...p.animations] } : {}),
    }
  }
  return {
    format: 1,
    id: decorId(draft),
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

/** A deep copy, so a host's draft is never drawn on behind its back. */
export function cloneDecor(draft: DecorDraft): DecorDraft {
  return {
    ...draft,
    props: draft.props.map(p => ({ ...p, perch: p.perch && { ...p.perch }, animations: [...p.animations], pixels: new Uint8ClampedArray(p.pixels) })),
  }
}

/** What `studio_open_decor` returns: the pack's `decor.json` text and each
 *  prop's sprite as RGBA, whole strip and all. */
export interface OpenedDecor {
  manifest: string
  sprites: { name: string; width: number; height: number; pixels: number[] }[]
}

const isSway = (value: unknown): value is Sway => SWAYS.some(s => s.value === value)
const isAnimation = (value: unknown): value is PropAnimation => PROP_ANIMATIONS.some(a => a.value === value)
const fraction = (value: unknown) => typeof value === 'number' && Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : null

/** An installed decor pack as a draft. `copy` starts a new pack from it (a new
 *  name, so a new id); `in_place` keeps its id so saving replaces it. Only what
 *  Studio can edit survives: an animated strip keeps its first frame, and a
 *  hand-tuned sway object becomes Soft. */
export function decorFromOpened(opened: OpenedDecor, mode: 'copy' | 'in_place'): DecorDraft {
  const manifest = JSON.parse(opened.manifest) as Record<string, any>
  const props: DecorProp[] = []
  for (const [name, raw] of Object.entries<any>(manifest.props ?? {})) {
    const sprite = opened.sprites.find(s => s.name === name)
    const width = Math.round(Number(raw?.width) || sprite?.width || 32)
    const height = Math.round(Number(raw?.height) || sprite?.height || 32)
    const pixels = new Uint8ClampedArray(width * height * 4)
    if (sprite) {
      // A strip lays frames side by side; the first frame is the left `width` columns.
      for (let y = 0; y < Math.min(height, sprite.height); y++) {
        const from = y * sprite.width * 4
        pixels.set(sprite.pixels.slice(from, from + Math.min(width, sprite.width) * 4), y * width * 4)
      }
    }
    const ledge = raw?.perch
    const [y, from, to] = [fraction(ledge?.y), fraction(ledge?.from), fraction(ledge?.to)]
    const animations = Array.isArray(raw?.animations) ? raw.animations.filter(isAnimation) : []
    props.push({
      name,
      width,
      height,
      sway: isSway(raw?.sway) ? raw.sway : 'soft',
      perch: y !== null && from !== null && to !== null ? { y, from, to } : null,
      animations: [...new Set<PropAnimation>(animations)],
      pixels,
    })
  }
  const title = String(manifest.name ?? '').trim()
  const author = String(manifest.author ?? '')
  return mode === 'copy'
    ? { name: `${title} copy`, author, props }
    : { id: String(manifest.id), name: title, author, props }
}

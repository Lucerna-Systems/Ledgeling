/**
 * Pet Studio's design model and the one place it turns into a pack manifest.
 * Every choice the editor offers comes from the fixed lists below, which mirror
 * the pack format's closed namespaces, so a design can only describe data.
 */

export type Movement = 'flier' | 'climber' | 'walker' | 'hopper'
export const FRAME = 48
export const MAX_FRAMES = 12

export const movements: { value: Movement; label: string; blurb: string }[] = [
  { value: 'flier', label: 'Flies', blurb: 'Perches on window tops and flies between them.' },
  { value: 'climber', label: 'Climbs', blurb: 'Scrambles up, around and under windows.' },
  { value: 'walker', label: 'Walks', blurb: 'Strolls along the tops of windows.' },
  { value: 'hopper', label: 'Hops', blurb: 'Walks, and hops across small gaps.' },
]

export interface ClipInfo { name: string; label: string; blurb: string; required?: boolean; fallback?: string; for?: Movement[]; group: 'basics' | 'moves' | 'moods' | 'assistant' }
/** The animations a pet can have. Only idle and move are required; the rest fall back. */
export const clips: ClipInfo[] = [
  { name: 'idle', label: 'Idle', blurb: 'Standing still, just being there.', required: true, group: 'basics' },
  { name: 'move', label: 'Move', blurb: 'Getting from place to place.', required: true, group: 'basics' },
  { name: 'climb', label: 'Climb', blurb: 'Going up or down the side of a window.', fallback: 'move', for: ['climber'], group: 'moves' },
  { name: 'jump', label: 'Jump', blurb: 'Leaping across a gap.', fallback: 'move', for: ['climber', 'hopper'], group: 'moves' },
  { name: 'fall', label: 'Fall', blurb: 'Dropping when a window vanishes.', fallback: 'move', for: ['climber', 'walker', 'hopper'], group: 'moves' },
  { name: 'takeoff', label: 'Take off', blurb: 'Lifting off into the air.', fallback: 'move', for: ['flier'], group: 'moves' },
  { name: 'land', label: 'Land', blurb: 'Touching down on a window.', fallback: 'idle', for: ['flier'], group: 'moves' },
  { name: 'sleep', label: 'Sleep', blurb: 'Dozing while you are away.', fallback: 'idle', group: 'moods' },
  { name: 'happy', label: 'Happy', blurb: 'A little celebration.', fallback: 'idle', group: 'moods' },
  { name: 'alert', label: 'Alert', blurb: 'Something caught its eye.', fallback: 'idle', group: 'moods' },
  { name: 'greet', label: 'Greet', blurb: 'Saying hello when you come back.', fallback: 'happy', group: 'moods' },
  { name: 'preen', label: 'Preen', blurb: 'Tidying itself up.', fallback: 'idle', group: 'moods' },
  { name: 'grabbed', label: 'Grabbed', blurb: 'Being carried by the pointer.', fallback: 'idle', group: 'moods' },
  { name: 'busy', label: 'Busy', blurb: 'Working on a task for you.', fallback: 'preen', group: 'assistant' },
  { name: 'attention', label: 'Needs you', blurb: 'Waiting for your answer on a task.', fallback: 'alert', group: 'assistant' },
]
export const clipInfo = (name: string) => clips.find(c => c.name === name)
export const clipsFor = (movement: Movement) => clips.filter(c => !c.for || c.for.includes(movement))

export const actions = [
  { value: '', label: 'stay put' },
  { value: 'move_to_random_surface', label: 'wander to a new spot' },
  { value: 'move_to_nearest_surface', label: 'hop to the nearest spot' },
  { value: 'patrol_current_surface', label: 'pace back and forth' },
  { value: 'move_toward_cursor', label: 'come to the pointer' },
  { value: 'flee_from_cursor', label: 'run from the pointer' },
  { value: 'return_to_home_surface', label: 'go back home' },
] as const

export const events = [
  { value: 'clicked', label: 'you click it' },
  { value: 'grabbed', label: 'you pick it up' },
  { value: 'released', label: 'you let it go' },
  { value: 'cursor_nearby', label: 'the pointer comes close' },
  { value: 'user_idle_long', label: 'you step away for a while' },
  { value: 'user_returned', label: 'you come back' },
  { value: 'long_session', label: 'you work a long stretch' },
  { value: 'break_taken', label: 'you take a break' },
  { value: 'surface_lost', label: 'its window disappears' },
  { value: 'surface_moved_fast', label: 'its window is flung around' },
  { value: 'app_category_changed', label: 'you switch kinds of app' },
  { value: 'hour_changed', label: 'a new hour starts' },
  { value: 'startup', label: 'Ledgeling starts' },
  { value: 'low_energy', label: 'it gets tired' },
] as const

export type SignalKind = 'number' | 'category' | 'bool'
export const signals: { value: string; label: string; kind: SignalKind; unit?: string; example: string }[] = [
  { value: 'idle_seconds', label: 'time since you last typed', kind: 'number', unit: 'sec', example: '240' },
  { value: 'session_minutes', label: 'time working without a break', kind: 'number', unit: 'min', example: '45' },
  { value: 'energy', label: 'its energy', kind: 'number', unit: '0–1', example: '0.25' },
  { value: 'mood', label: 'its mood', kind: 'number', unit: '0–1', example: '0.5' },
  { value: 'hour', label: 'the hour of day', kind: 'number', unit: '0–23', example: '22' },
  { value: 'cursor_distance', label: 'distance to the pointer', kind: 'number', unit: 'px', example: '120' },
  { value: 'surface_count', label: 'places it can stand', kind: 'number', example: '3' },
  { value: 'app_category', label: 'the kind of app in front', kind: 'category', example: 'code' },
  { value: 'is_dragging', label: 'you are dragging it', kind: 'bool', example: 'true' },
]
export const categories = ['code', 'browser', 'comms', 'media', 'creative', 'game', 'other']
export const numberOps = [
  { value: '>', label: 'is more than' }, { value: '<', label: 'is less than' },
  { value: '>=', label: 'is at least' }, { value: '<=', label: 'is at most' },
  { value: '==', label: 'is' }, { value: '!=', label: 'is not' },
]
export const equalityOps = [{ value: '==', label: 'is' }, { value: '!=', label: 'is not' }]

export interface Condition { signal: string; op: string; value: string }
export interface Transition { to: string; weight: number; when: Condition[] }
export interface StateDraft { key: number; name: string; clip: string; action: string; duration: [number, number] | null; transitions: Transition[] }
export interface ReactionDraft { key: number; on: string; goto: string; cooldown: number | null }
export interface ClipDraft { frames: Uint8ClampedArray[]; fps: number; loop: boolean }
export interface Draft {
  name: string
  author: string
  description: string
  movement: Movement
  clips: Record<string, ClipDraft>
  initial: string
  states: StateDraft[]
  reactions: ReactionDraft[]
}

let keys = 0
export const nextKey = () => ++keys
export const blankFrame = () => new Uint8ClampedArray(FRAME * FRAME * 4)
export const emptyClip = (): ClipDraft => ({ frames: [blankFrame()], fps: 8, loop: true })
export const drawn = (frame: Uint8ClampedArray) => frame.some((value, i) => i % 4 === 3 && value > 0)
export const clipDrawn = (clip: ClipDraft | undefined) => !!clip && clip.frames.some(drawn)

/** A pet that already behaves well: rests, wanders, naps when you leave and greets you back. */
export function starterDraft(movement: Movement = 'walker'): Draft {
  const state = (name: string, clip: string, action: string, duration: [number, number] | null, transitions: Transition[]): StateDraft =>
    ({ key: nextKey(), name, clip, action, duration, transitions })
  return {
    name: 'My Pet',
    author: '',
    description: '',
    movement,
    clips: { idle: emptyClip(), move: emptyClip() },
    initial: 'rest',
    states: [
      state('rest', 'idle', '', [5, 15], [
        { to: 'wander', weight: 3, when: [] },
        { to: 'nap', weight: 5, when: [{ signal: 'idle_seconds', op: '>', value: '240' }] },
      ]),
      state('wander', 'move', 'move_to_random_surface', null, [{ to: 'rest', weight: 1, when: [] }]),
      state('nap', 'sleep', '', [30, 120], [{ to: 'say_hi', weight: 1, when: [{ signal: 'idle_seconds', op: '<', value: '10' }] }]),
      state('say_hi', 'happy', '', [2, 3], [{ to: 'rest', weight: 1, when: [] }]),
    ],
    reactions: [
      { key: nextKey(), on: 'clicked', goto: 'say_hi', cooldown: 3 },
      { key: nextKey(), on: 'user_returned', goto: 'say_hi', cooldown: 60 },
      { key: nextKey(), on: 'user_idle_long', goto: 'nap', cooldown: null },
      { key: nextKey(), on: 'surface_lost', goto: 'wander', cooldown: null },
    ],
  }
}

/** State names are manifest keys; keep them to words the format and people both read easily. */
export function stateName(text: string) {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '').slice(0, 32) || 'state'
}
export function renameState(draft: Draft, from: string, to: string) {
  if (from === to) return
  for (const state of draft.states) {
    if (state.name === from) state.name = to
    for (const t of state.transitions) if (t.to === from) t.to = to
  }
  for (const r of draft.reactions) if (r.goto === from) r.goto = to
  if (draft.initial === from) draft.initial = to
}

export function slug(name: string) {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '') || 'my-pet'
}

const locomotion: Record<Movement, Record<string, unknown>> = {
  flier: { surfaces: ['window_top', 'monitor_top', 'taskbar_top'], speed: { surface: 45, air: 220 }, gravity: 0, air_style: 'flap_arc' },
  climber: { surfaces: ['window_any', 'monitor_any', 'taskbar_top'], speed: { surface: 110, climb: 85 }, jump: { enabled: true, max_gap: 160, arc_height: 60 }, gravity: 1 },
  walker: { surfaces: ['window_top', 'taskbar_top'], speed: { surface: 80 }, gravity: 1 },
  hopper: { surfaces: ['window_top', 'taskbar_top'], speed: { surface: 90 }, jump: { enabled: true, max_gap: 90, arc_height: 50 }, gravity: 1 },
}

export function conditionText(when: Condition[]) {
  return when.map(c => `${c.signal} ${c.op} ${c.value.trim()}`).join(' && ')
}

/** Problems worth fixing before saving, in plain words. The backend validator still has the final say. */
export function problems(draft: Draft): string[] {
  const out: string[] = []
  if (!draft.name.trim()) out.push('Give your pet a name.')
  for (const required of ['idle', 'move']) if (!clipDrawn(draft.clips[required])) out.push(`Draw at least one frame of the ${clipInfo(required)!.label} animation.`)
  const names = new Set(draft.states.map(s => s.name))
  if (names.size !== draft.states.length) out.push('Two behaviours share a name. Give each its own.')
  if (!names.has(draft.initial)) out.push('Pick which behaviour your pet starts with.')
  for (const state of draft.states) {
    if (state.duration && (state.duration[0] < 0 || state.duration[1] < state.duration[0])) out.push(`“${state.name}” lasts a shorter maximum than minimum time.`)
    for (const t of state.transitions) {
      if (!names.has(t.to)) out.push(`“${state.name}” goes to a behaviour that no longer exists.`)
      for (const c of t.when) {
        const kind = signals.find(s => s.value === c.signal)?.kind
        if (kind === 'number' && (c.value.trim() === '' || Number.isNaN(Number(c.value)))) out.push(`A condition in “${state.name}” needs a number.`)
      }
    }
  }
  for (const r of draft.reactions) if (!names.has(r.goto)) out.push('A reaction goes to a behaviour that no longer exists.')
  return [...new Set(out)]
}

/** The manifest exactly as it is written to pet.json. */
export function buildManifest(draft: Draft) {
  const animations: Record<string, Record<string, unknown>> = {}
  for (const [name, clip] of Object.entries(draft.clips)) {
    if (!clipDrawn(clip)) continue
    animations[name] = { file: `sprites/${name}.png`, frames: clip.frames.length, fps: clip.fps, ...(clip.loop ? {} : { loop: false }) }
  }
  const states: Record<string, Record<string, unknown>> = {}
  for (const state of draft.states) {
    states[state.name] = {
      clip: state.clip,
      ...(state.action ? { action: state.action } : {}),
      ...(state.duration ? { duration: state.duration } : {}),
      transitions: state.transitions.map(t => ({ to: t.to, weight: t.weight, ...(t.when.length ? { when: conditionText(t.when) } : {}) })),
    }
  }
  return {
    format: 1,
    id: `com.studio.${slug(draft.name)}`,
    name: draft.name.trim(),
    author: draft.author.trim() || 'Me',
    version: '1.0.0',
    description: draft.description.trim(),
    license: 'CC-BY-4.0',
    tags: ['studio'],
    sprite: { width: FRAME, height: FRAME, anchor: { x: 0.5, y: 1 }, default_facing: 'right', surface_orientation: draft.movement === 'flier' ? 'upright' : 'align' },
    locomotion: { class: draft.movement, ...locomotion[draft.movement], min_surface_length: FRAME },
    animations,
    behavior: { initial: draft.initial, states },
    reactions: draft.reactions.map(r => ({ on: r.on, goto: r.goto, ...(r.cooldown ? { cooldown: r.cooldown } : {}) })),
  }
}

/** One horizontal strip of equal-width frames, as the pack format stores animations. */
export function strip(frames: Uint8ClampedArray[]) {
  const width = FRAME * frames.length
  const pixels = new Uint8ClampedArray(width * FRAME * 4)
  frames.forEach((frame, index) => {
    for (let y = 0; y < FRAME; y++) {
      pixels.set(frame.subarray(y * FRAME * 4, (y + 1) * FRAME * 4), (y * width + index * FRAME) * 4)
    }
  })
  return { width, height: FRAME, pixels }
}

/** The save payload for `save_studio_pack`. */
export function payload(draft: Draft, preview: { width: number; height: number; pixels: Uint8ClampedArray }) {
  const strips = Object.entries(draft.clips)
    .filter(([, clip]) => clipDrawn(clip))
    .map(([name, clip]) => { const s = strip(clip.frames); return { name, width: s.width, height: s.height, pixels: Array.from(s.pixels) } })
  return {
    manifest: JSON.stringify(buildManifest(draft), null, 2),
    strips,
    preview: { name: 'preview', width: preview.width, height: preview.height, pixels: Array.from(preview.pixels) },
  }
}

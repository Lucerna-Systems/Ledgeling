/**
 * The Studio's AI helper: turns "what should my pet do?" into a draft of its
 * behaviours. The model only ever answers with JSON, and everything it says is
 * checked against the same closed lists the block editor offers before it
 * touches the design, so a wrong answer can at worst look wrong, never add
 * anything the pack format does not already allow.
 */
import {
  actions, categories, clipsFor, equalityOps, events, nextKey, numberOps, signals, stateName,
  type Draft, type ReactionDraft, type StateDraft, type Transition,
} from './model'

const MAX_STATES = 12
const MAX_REACTIONS = 14

/** The prompt. Only the behaviour outline and the request leave the Studio, never the drawings. */
export function behaviourPrompt(draft: Draft, request: string): string {
  const clipList = clipsFor(draft.movement).map(c => `${c.name}${draft.clips[c.name] ? '' : ' (not drawn)'}`).join(', ')
  const signalList = signals.map(s => `${s.value} (${s.kind === 'number' ? `number, e.g. ${s.example}` : s.kind === 'bool' ? 'true/false' : `one of ${categories.join('/')}`}) — ${s.label}`).join('\n')
  const current = { initial: draft.initial, states: draft.states.map(s => serialiseState(s)), reactions: draft.reactions.map(r => ({ on: r.on, goto: r.goto, cooldown: r.cooldown })) }
  return [
    `You are helping design the behaviour of a desktop pet that ${draft.movement === 'flier' ? 'flies' : draft.movement === 'climber' ? 'climbs' : draft.movement === 'hopper' ? 'hops' : 'walks'} along window edges.`,
    'Its behaviour is a set of named states. Each state plays an animation, may run an action, may last a random number of seconds, and lists weighted next states, each optionally guarded by conditions. Reactions jump to a state when an event happens.',
    `Animations you may use: ${clipList}`,
    `Actions you may use (or "" for none): ${actions.filter(a => a.value).map(a => a.value).join(', ')}`,
    `Events for reactions: ${events.map(e => e.value).join(', ')}`,
    `Signals for conditions, compared with ${[...numberOps, ...equalityOps].map(o => o.value).filter((v, i, all) => all.indexOf(v) === i).join(' ')} (only == and != for non-numbers):\n${signalList}`,
    `Use at most ${MAX_STATES} states. State names are short lowercase words with underscores.`,
    'Reply with ONLY one JSON object, no prose and no code fence, shaped exactly like:',
    '{"initial":"rest","states":[{"name":"rest","clip":"idle","action":"","duration":[5,15],"next":[{"to":"wander","weight":3,"when":[{"signal":"idle_seconds","op":">","value":"240"}]}]}],"reactions":[{"on":"clicked","goto":"rest","cooldown":3}]}',
    'Use null for duration when a state should last until its action finishes. Keep anything in the current design the request does not ask to change.',
    `The current design:\n${JSON.stringify(current)}`,
    `The request: ${request.trim()}`,
  ].join('\n\n')
}

function serialiseState(s: StateDraft) {
  return {
    name: s.name, clip: s.clip, action: s.action, duration: s.duration,
    next: s.transitions.map(t => ({ to: t.to, weight: t.weight, when: t.when })),
  }
}

export interface Suggestion { initial: string; states: StateDraft[]; reactions: ReactionDraft[]; dropped: number }

/** The JSON object in a reply, tolerating a code fence or a sentence around it. */
function extractObject(text: string): unknown {
  const start = text.indexOf('{')
  const end = text.lastIndexOf('}')
  if (start < 0 || end <= start) throw new Error('The reply did not contain a behaviour design.')
  try { return JSON.parse(text.slice(start, end + 1)) } catch { throw new Error('The reply was not a readable behaviour design.') }
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v)
const finite = (v: unknown) => typeof v === 'number' && Number.isFinite(v) ? v : typeof v === 'string' && v.trim() !== '' && Number.isFinite(Number(v)) ? Number(v) : null

/** Reads a reply into a design, keeping only what the block editor could have produced. */
export function parseSuggestion(text: string, draft: Draft): Suggestion {
  const raw = extractObject(text)
  if (!isRecord(raw) || !Array.isArray(raw.states)) throw new Error('The reply had no behaviours in it.')
  let dropped = 0
  const clipNames = new Set(clipsFor(draft.movement).map(c => c.name))
  const actionNames = new Set<string>(actions.map(a => a.value))
  const eventNames = new Set<string>(events.map(e => e.value))

  const names = new Map<string, string>()
  for (const item of raw.states.slice(0, MAX_STATES)) {
    if (!isRecord(item) || typeof item.name !== 'string') { dropped++; continue }
    let name = stateName(item.name)
    const base = name
    for (let n = 2; [...names.values()].includes(name); n++) name = `${base}_${n}`
    names.set(item.name, name)
  }
  const resolve = (v: unknown) => typeof v === 'string' ? names.get(v) ?? ([...names.values()].includes(stateName(v)) ? stateName(v) : undefined) : undefined

  const condition = (c: unknown) => {
    if (!isRecord(c)) return null
    const info = signals.find(s => s.value === c.signal)
    if (!info) return null
    const ops = info.kind === 'number' ? numberOps : equalityOps
    const op = ops.find(o => o.value === c.op)?.value
    const value = String(c.value ?? '').trim().toLowerCase()
    if (!op) return null
    if (info.kind === 'number' && finite(value) === null) return null
    if (info.kind === 'category' && !categories.includes(value)) return null
    if (info.kind === 'bool' && value !== 'true' && value !== 'false') return null
    return { signal: info.value, op, value }
  }

  const states: StateDraft[] = []
  for (const item of raw.states.slice(0, MAX_STATES)) {
    if (!isRecord(item) || typeof item.name !== 'string') continue
    const name = names.get(item.name)!
    const clip = typeof item.clip === 'string' && clipNames.has(item.clip) ? item.clip : 'idle'
    if (clip !== item.clip) dropped++
    const action = typeof item.action === 'string' && actionNames.has(item.action) ? item.action : ''
    if (action !== (item.action ?? '')) dropped++
    let duration: [number, number] | null = null
    if (Array.isArray(item.duration)) {
      const [lo, hi] = [finite(item.duration[0]), finite(item.duration[1])]
      if (lo !== null && hi !== null && lo >= 0 && hi >= lo) duration = [lo, hi]
      else dropped++
    }
    const transitions: Transition[] = []
    for (const t of Array.isArray(item.next) ? item.next : []) {
      const to = isRecord(t) ? resolve(t.to) : undefined
      if (!isRecord(t) || !to) { dropped++; continue }
      const when = (Array.isArray(t.when) ? t.when : []).map(condition)
      dropped += when.filter(c => c === null).length
      const weight = finite(t.weight)
      transitions.push({ to, weight: weight !== null && weight >= 0 ? weight : 1, when: when.filter(c => c !== null) })
    }
    states.push({ key: nextKey(), name, clip, action, duration, transitions })
  }
  if (!states.length) throw new Error('The reply had no usable behaviours in it.')

  const initial = resolve(raw.initial) ?? states[0]!.name
  // A state with nowhere to go would strand the pet; send it back to where it starts.
  for (const s of states) if (!s.transitions.length) s.transitions.push({ to: initial, weight: 1, when: [] })

  const reactions: ReactionDraft[] = []
  for (const r of (Array.isArray(raw.reactions) ? raw.reactions : []).slice(0, MAX_REACTIONS)) {
    const goto = isRecord(r) ? resolve(r.goto) : undefined
    if (!isRecord(r) || typeof r.on !== 'string' || !eventNames.has(r.on) || !goto) { dropped++; continue }
    const cooldown = finite(r.cooldown)
    reactions.push({ key: nextKey(), on: r.on, goto, cooldown: cooldown !== null && cooldown > 0 ? cooldown : null })
  }
  return { initial, states, reactions, dropped }
}

/** A copy of the behaviour half of a design, so an applied suggestion can be undone. */
export function snapshot(draft: Draft) {
  // JSON, not structuredClone: the draft is a reactive proxy, which structuredClone refuses.
  return JSON.parse(JSON.stringify({ initial: draft.initial, states: draft.states, reactions: draft.reactions })) as { initial: string; states: StateDraft[]; reactions: ReactionDraft[] }
}
export function restore(draft: Draft, saved: ReturnType<typeof snapshot>) {
  draft.initial = saved.initial
  draft.states = saved.states
  draft.reactions = saved.reactions
}
export function apply(draft: Draft, s: Suggestion) {
  draft.initial = s.initial
  draft.states = s.states
  draft.reactions = s.reactions
}

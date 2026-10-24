import { ref } from 'vue'
import { call } from '../state'
import { FRAME, draftFromPack, reserveKeys, starterDraft, type ClipDraft, type Draft, type OpenedPack } from './model'

/** The design in progress; module-level so leaving the Studio and coming back keeps the work. */
export const draft = ref<Draft>(starterDraft())

/** Load an installed pack into the editor; `copy` starts a new pet from it instead of editing it in place. */
export async function openInStudio(id: string, copy: boolean): Promise<boolean> {
  const opened = await call<OpenedPack>('studio_open_pack', { id }, false)
  if (!opened) return false
  draft.value = draftFromPack(opened, { copy })
  return true
}

const FRAME_BYTES = FRAME * FRAME * 4

function toBase64(bytes: Uint8ClampedArray): string {
  let text = ''
  for (let i = 0; i < bytes.length; i += 0x8000) text += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  return btoa(text)
}
function fromBase64(text: string): Uint8ClampedArray | null {
  try {
    const raw = atob(text)
    if (raw.length !== FRAME_BYTES) return null
    return Uint8ClampedArray.from(raw, c => c.charCodeAt(0))
  } catch { return null }
}

/** A JSON-safe copy of a draft: pixel frames become base64 strings. */
export function serializeDraft(value: Draft): unknown {
  const clips: Record<string, unknown> = {}
  for (const [name, clip] of Object.entries(value.clips)) clips[name] = { ...clip, frames: clip.frames.map(toBase64) }
  return { ...value, clips }
}

/** The inverse of `serializeDraft`; anything malformed yields `null` so the caller keeps its current draft. */
export function restoreDraft(raw: unknown): Draft | null {
  if (!raw || typeof raw !== 'object') return null
  const source = raw as Record<string, any>
  const base = starterDraft()
  for (const key of Object.keys(base)) if (!(key in source)) return null
  if (!source.clips || typeof source.clips !== 'object' || !Array.isArray(source.states) || !Array.isArray(source.reactions)) return null
  const clips: Record<string, ClipDraft> = {}
  for (const [name, clip] of Object.entries<any>(source.clips)) {
    if (!clip || !Array.isArray(clip.frames)) return null
    const frames = clip.frames.map((f: unknown) => (typeof f === 'string' ? fromBase64(f) : null))
    if (frames.some((f: unknown) => !f)) return null
    clips[name] = { frames, fps: Number(clip.fps) || 8, loop: clip.loop !== false }
  }
  const keys = [...source.states, ...source.reactions].map((item: any) => Number(item?.key) || 0)
  reserveKeys(Math.max(0, ...keys))
  return { ...(source as Draft), clips }
}

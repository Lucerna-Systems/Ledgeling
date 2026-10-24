import { ref } from 'vue'
import { call } from '../state'
import { draftFromPack, starterDraft, type Draft, type OpenedPack } from './model'

/** The design in progress; module-level so leaving the Studio and coming back keeps the work. */
export const draft = ref<Draft>(starterDraft())

/** Load an installed pack into the editor; `copy` starts a new pet from it instead of editing it in place. */
export async function openInStudio(id: string, copy: boolean): Promise<boolean> {
  const opened = await call<OpenedPack>('studio_open_pack', { id }, false)
  if (!opened) return false
  draft.value = draftFromPack(opened, { copy })
  return true
}

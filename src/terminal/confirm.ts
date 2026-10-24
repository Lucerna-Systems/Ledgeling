import { shallowRef } from 'vue'

export interface Question { message: string; action: string; danger: boolean; answer(yes: boolean): void }

/** The question on screen, if any. */
export const question = shallowRef<Question | null>(null)

/** Ask before something that cannot be undone. The webview's own `confirm()`
 *  cannot be used: Tauri's macOS webview has no dialog for it and answers no. */
export function ask(message: string, action: string, danger = true): Promise<boolean> {
  question.value?.answer(false)
  return new Promise(resolve => {
    question.value = { message, action, danger, answer: yes => { question.value = null; resolve(yes) } }
  })
}

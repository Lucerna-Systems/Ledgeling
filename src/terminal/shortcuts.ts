import type { Direction } from './layout'

export const mac = navigator.userAgent.includes('Mac')

/** iTerm's split keys on macOS (⌘D, ⇧⌘D); Terminator's elsewhere (Ctrl+Shift+E,
 *  Ctrl+Shift+O), since a bare Ctrl+D is end-of-input in every shell. */
export function splitShortcut(event: KeyboardEvent): Direction | null {
  if (event.type !== 'keydown' || event.altKey) return null
  const key = event.key.toLowerCase()
  if (mac) {
    if (!event.metaKey || event.ctrlKey || key !== 'd') return null
    return event.shiftKey ? 'column' : 'row'
  }
  if (!event.ctrlKey || !event.shiftKey || event.metaKey) return null
  return key === 'e' ? 'row' : key === 'o' ? 'column' : null
}

export const splitHint = (dir: Direction) => mac
  ? (dir === 'row' ? '⌘D' : '⇧⌘D')
  : (dir === 'row' ? 'Ctrl+Shift+E' : 'Ctrl+Shift+O')

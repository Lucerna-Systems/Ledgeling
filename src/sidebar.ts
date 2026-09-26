import { ref, watch } from 'vue'

const KEY = 'perch:sidebar'
export const SIDEBAR_MIN = 210
export const SIDEBAR_MAX = 440
export const SIDEBAR_DEFAULT = 264

const clamp = (width: number) => Math.round(Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, width)))

function stored(): { width: number; collapsed: boolean } {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? '{}') as { width?: number; collapsed?: boolean }
    return { width: clamp(saved.width ?? SIDEBAR_DEFAULT), collapsed: !!saved.collapsed }
  } catch { return { width: SIDEBAR_DEFAULT, collapsed: false } }
}

/** Sidebar width and collapsed state; a per-machine layout preference, so it lives in local storage. */
export function useSidebar() {
  const initial = stored()
  const width = ref(initial.width)
  const collapsed = ref(initial.collapsed)
  const resizing = ref(false)
  watch([width, collapsed], () => {
    try { localStorage.setItem(KEY, JSON.stringify({ width: width.value, collapsed: collapsed.value })) } catch { /* private mode */ }
  })

  function startResize(event: PointerEvent) {
    const handle = event.currentTarget as HTMLElement
    const startX = event.clientX
    const startWidth = width.value
    handle.setPointerCapture(event.pointerId)
    resizing.value = true
    const move = (e: PointerEvent) => { width.value = clamp(startWidth + e.clientX - startX) }
    const end = () => {
      resizing.value = false
      handle.removeEventListener('pointermove', move)
      handle.removeEventListener('pointerup', end)
      handle.removeEventListener('pointercancel', end)
    }
    handle.addEventListener('pointermove', move)
    handle.addEventListener('pointerup', end)
    handle.addEventListener('pointercancel', end)
  }
  function nudge(delta: number) { width.value = clamp(width.value + delta) }
  function reset() { width.value = SIDEBAR_DEFAULT }

  return { width, collapsed, resizing, startResize, nudge, reset }
}

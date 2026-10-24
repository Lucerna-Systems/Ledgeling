import { ref } from 'vue'

/** Editor and terminal colours read from the live CSS tokens, so they follow
 *  the app's light/dark mode and accent without a second palette. */
export interface Palette {
  dark: boolean
  background: string
  surface: string
  ink: string
  muted: string
  line: string
  accent: string
  accentSoft: string
  danger: string
  live: string
  warn: string
}

function token(name: string, fallback: string) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return value || fallback
}

/** Monaco and xterm want concrete #rrggbb colours, not `color-mix()`. */
function hex(color: string) {
  if (/^#[0-9a-f]{6}$/i.test(color)) return color
  if (/^#[0-9a-f]{3}$/i.test(color)) return '#' + [...color.slice(1)].map(c => c + c).join('')
  const probe = document.createElement('span')
  probe.style.color = color
  document.body.appendChild(probe)
  const rgb = getComputedStyle(probe).color.match(/\d+(\.\d+)?/g)?.map(Number) ?? [0, 0, 0]
  probe.remove()
  return '#' + rgb.slice(0, 3).map(v => Math.round(v).toString(16).padStart(2, '0')).join('')
}

export function readPalette(): Palette {
  const dark = document.documentElement.dataset.theme === 'dark'
  return {
    dark,
    background: hex(token('--surface', dark ? '#171a22' : '#ffffff')),
    surface: hex(token('--soft', dark ? '#1f232d' : '#f0f2f7')),
    ink: hex(token('--ink', dark ? '#e8eaf0' : '#171b26')),
    muted: hex(token('--muted', '#5f6879')),
    line: hex(token('--line', '#e4e7ee')),
    accent: hex(token('--accent', '#5b5bd6')),
    accentSoft: hex(token('--accent-soft', '#ededfa')),
    danger: hex(token('--danger', '#c2413a')),
    live: hex(token('--status-live', '#22a06b')),
    warn: hex(token('--status-waiting-ink', '#8b650f')),
  }
}

/** Changes whenever the theme or accent is repainted. */
export const palette = ref<Palette | null>(null)
let observer: MutationObserver | null = null
export function trackPalette() {
  if (observer) return
  palette.value = readPalette()
  observer = new MutationObserver(() => { palette.value = readPalette() })
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'style'] })
}

export function terminalTheme(p: Palette) {
  return {
    background: p.background,
    foreground: p.ink,
    cursor: p.accent,
    cursorAccent: p.background,
    selectionBackground: p.accent + '44',
    black: p.dark ? '#1f232d' : '#2b3040',
    brightBlack: p.muted,
    red: p.danger,
    brightRed: p.dark ? '#f07a72' : '#d9544b',
    green: p.live,
    brightGreen: p.dark ? '#4fd49a' : '#2bb57a',
    yellow: p.dark ? '#e0b54a' : '#a87c12',
    brightYellow: p.dark ? '#f1cc6b' : '#c9951a',
    blue: p.dark ? '#6f9cf0' : '#3565c9',
    brightBlue: p.dark ? '#95b6f5' : '#4f7fe0',
    magenta: p.dark ? '#b88ae8' : '#8a4fd0',
    brightMagenta: p.dark ? '#cfa9f2' : '#a36be0',
    cyan: p.dark ? '#5cc6d6' : '#1b8fa3',
    brightCyan: p.dark ? '#86d9e5' : '#27a9bf',
    white: p.dark ? '#c9ceda' : '#d6dae3',
    brightWhite: p.dark ? '#ffffff' : '#f5f6fa',
  }
}

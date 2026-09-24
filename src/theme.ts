import { watch } from 'vue'
import { settings } from './state'
import type { ThemeMode } from './types'

/**
 * Colour theme application for the companion UI. The resolved mode is a
 * `data-theme` attribute (the dark token block lives in style.css); the accent
 * and its variants are inline custom properties, so presets and free-form
 * colours share one path. Preset variants are hand-tuned here rather than in
 * CSS so the swatch list and the applied values can never drift apart.
 */

export interface AccentPreset {
  id: string
  label: string
  base: string
  hoverLight: string
  hoverDark: string
  softLight: string
  softDark: string
}

export const ACCENT_PRESETS: AccentPreset[] = [
  { id: 'moss', label: 'Moss', base: '#31725b', hoverLight: '#245b47', hoverDark: '#3f8f70', softLight: '#e6f2eb', softDark: '#1d2b27' },
  { id: 'ocean', label: 'Ocean', base: '#2e6da3', hoverLight: '#1f5586', hoverDark: '#4a8ac2', softLight: '#e4eef8', softDark: '#1c2733' },
  { id: 'plum', label: 'Plum', base: '#70518f', hoverLight: '#593d73', hoverDark: '#8f6ead', softLight: '#f0e9f7', softDark: '#292233' },
  { id: 'clay', label: 'Clay', base: '#b05a3f', hoverLight: '#93452f', hoverDark: '#c4795f', softLight: '#f8ece6', softDark: '#332622' },
  { id: 'indigo', label: 'Indigo', base: '#46557b', hoverLight: '#354263', hoverDark: '#61719c', softLight: '#e9ecf5', softDark: '#232838' },
  { id: 'honey', label: 'Honey', base: '#b08421', hoverLight: '#8f6b17', hoverDark: '#c9a044', softLight: '#f8f0dc', softDark: '#322b1d' },
  { id: 'rose', label: 'Rose', base: '#ab4a62', hoverLight: '#8c364b', hoverDark: '#c46b80', softLight: '#f8e7ec', softDark: '#342328' },
  { id: 'teal', label: 'Teal', base: '#1f7a78', hoverLight: '#135f5d', hoverDark: '#2f9d9a', softLight: '#e1f2f1', softDark: '#1a2b2b' },
]

export const DEFAULT_ACCENT = ACCENT_PRESETS[0].base
const CACHE_KEY = 'perch:appearance'

export function presetFor(accent: string): AccentPreset | undefined {
  const normalised = accent.toLowerCase()
  return ACCENT_PRESETS.find(preset => preset.base === normalised)
}

function hexToHsl(hex: string): [number, number, number] {
  const value = hex.replace('#', '')
  const r = parseInt(value.slice(0, 2), 16) / 255
  const g = parseInt(value.slice(2, 4), 16) / 255
  const b = parseInt(value.slice(4, 6), 16) / 255
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  const h = max === r
    ? ((g - b) / d + (g < b ? 6 : 0)) / 6
    : max === g ? ((b - r) / d + 2) / 6 : ((r - g) / d + 4) / 6
  return [h * 360, s, l]
}

function hslToHex(h: number, s: number, l: number): string {
  const hue = (h % 360 + 360) % 360
  const c = (1 - Math.abs(2 * l - 1)) * s
  const x = c * (1 - Math.abs(((hue / 60) % 2) - 1))
  const m = l - c / 2
  const [r, g, b] = hue < 60 ? [c, x, 0] : hue < 120 ? [x, c, 0] : hue < 180 ? [0, c, x]
    : hue < 240 ? [0, x, c] : hue < 300 ? [x, 0, c] : [c, 0, x]
  const channel = (v: number) => Math.round(Math.min(1, Math.max(0, v + m)) * 255).toString(16).padStart(2, '0')
  return `#${channel(r)}${channel(g)}${channel(b)}`
}

export interface AccentVariants { base: string; hover: string; soft: string; onAccent: string }

/** Hover and soft-background variants for the resolved mode; tuned presets
 * win, anything else is derived by simple HSL moves. */
export function accentVariants(accent: string, mode: 'light' | 'dark'): AccentVariants {
  const preset = presetFor(accent)
  if (preset) {
    return {
      base: preset.base,
      hover: mode === 'light' ? preset.hoverLight : preset.hoverDark,
      soft: mode === 'light' ? preset.softLight : preset.softDark,
      onAccent: onAccentFor(preset.base),
    }
  }
  const [h, s, l] = hexToHsl(accent)
  const lifted = mode === 'dark' && l < 0.22 ? hslToHex(h, s, l + (1 - l) * 0.35) : accent
  const [, , ll] = hexToHsl(lifted)
  return {
    base: lifted,
    hover: mode === 'light'
      ? hslToHex(h, s, Math.max(l * 0.82, 0.16))
      : hslToHex(h, s, Math.min(ll + (1 - ll) * 0.22, 0.92)),
    soft: mode === 'light' ? hslToHex(h, s * 0.55, 0.94) : hslToHex(h, s * 0.45, 0.15),
    onAccent: onAccentFor(lifted),
  }
}

/** Text colour on accent-filled controls; pale accents need dark text. */
function onAccentFor(hex: string): string {
  const value = hex.replace('#', '')
  const r = parseInt(value.slice(0, 2), 16) / 255
  const g = parseInt(value.slice(2, 4), 16) / 255
  const b = parseInt(value.slice(4, 6), 16) / 255
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b
  return luminance < 0.4 ? '#ffffff' : '#1a211d'
}

function systemPrefersDark(): boolean {
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

function currentTheme(): ThemeMode {
  return settings.value?.theme ?? 'system'
}

function currentAccent(): string {
  return settings.value?.accent ?? DEFAULT_ACCENT
}

export function applyTheme(theme: ThemeMode, accent: string) {
  const mode: 'light' | 'dark' = theme === 'system' ? (systemPrefersDark() ? 'dark' : 'light') : theme
  const root = document.documentElement
  root.dataset.theme = mode
  const { base, hover, soft, onAccent } = accentVariants(accent, mode)
  root.style.setProperty('--accent', base)
  root.style.setProperty('--accent-hover', hover)
  root.style.setProperty('--accent-soft', soft)
  root.style.setProperty('--on-accent', onAccent)
  // Repainted synchronously on the next launch, before the first snapshot
  // poll returns, so a dark-mode user never sees a light flash.
  try { localStorage.setItem(CACHE_KEY, JSON.stringify({ theme, accent })) } catch { /* private mode */ }
}

let started = false

/** Called once from main.ts before mount. */
export function initTheme() {
  if (started) return
  started = true
  try {
    const cached = localStorage.getItem(CACHE_KEY)
    if (cached) {
      const { theme, accent } = JSON.parse(cached) as { theme: ThemeMode; accent: string }
      applyTheme(theme, accent)
    }
  } catch { /* fall through to settings-driven application */ }
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (currentTheme() === 'system') applyTheme('system', currentAccent())
  })
  watch(
    () => [settings.value?.theme, settings.value?.accent] as const,
    ([theme, accent], previous) => {
      if (theme && accent && (!previous?.[0] || !previous?.[1] || previous[0] !== theme || previous[1] !== accent)) {
        applyTheme(theme, accent)
      }
    },
    { immediate: true },
  )
}
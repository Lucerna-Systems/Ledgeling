// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import { ACCENT_PRESETS, accentVariants, applyTheme, presetFor } from './theme'
import { snapshot } from './state'
import type { Snapshot } from './types'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

type ChangeListener = (event: { matches: boolean }) => void

/** jsdom has no matchMedia; fake just enough for theme resolution. */
function fakeMatchMedia(matches: boolean) {
  const listeners: ChangeListener[] = []
  window.matchMedia = ((query: string) => ({
    matches,
    addEventListener: (_: string, listener: ChangeListener) => listeners.push(listener),
  })) as unknown as typeof window.matchMedia
  return {
    setMatches: (value: boolean) => {
      matches = value
      listeners.forEach(listener => listener({ matches }))
    },
  }
}

const root = () => document.documentElement

// This jsdom setup ships without a localStorage implementation, so install a
// minimal one both where the app reads it (global) and where jsdom keeps it.
function installStorage() {
  const store = new Map<string, string>()
  const storage = {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, value),
    removeItem: (key: string) => void store.delete(key),
    clear: () => store.clear(),
  }
  Object.defineProperty(globalThis, 'localStorage', { value: storage, configurable: true })
  Object.defineProperty(window, 'localStorage', { value: storage, configurable: true })
}
installStorage()

beforeEach(() => {
  // initTheme runs once per module instance, so every initTheme test needs a
  // fresh registry — which also means a fresh `state` to drive it with.
  vi.resetModules()
  localStorage.clear()
  root().dataset.theme = ''
  root().style.cssText = ''
  snapshot.value = null
})

describe('applyTheme', () => {
  it('sets the resolved mode and the tuned variants of a preset accent', () => {
    fakeMatchMedia(true)
    applyTheme('dark', '#5b5bd6')
    expect(root().dataset.theme).toBe('dark')
    expect(root().style.getPropertyValue('--accent')).toBe('#5b5bd6')
    expect(root().style.getPropertyValue('--accent-hover')).toBe('#7c7cde')
    expect(root().style.getPropertyValue('--accent-soft')).toBe('#1f1f33')
    expect(root().style.getPropertyValue('--accent-2')).toBe('#8f4fd6')
    expect(root().style.getPropertyValue('--on-accent')).toBe('#ffffff')
  })

  it('resolves system mode from prefers-color-scheme', () => {
    const media = fakeMatchMedia(false)
    applyTheme('system', '#5b5bd6')
    expect(root().dataset.theme).toBe('light')
    media.setMatches(true)
    applyTheme('system', '#5b5bd6')
    expect(root().dataset.theme).toBe('dark')
  })

  it('caches the applied pair for the next launch', () => {
    fakeMatchMedia(false)
    applyTheme('dark', '#5b5bd6')
    expect(JSON.parse(localStorage.getItem('ledgeling:appearance') ?? '{}')).toEqual({
      theme: 'dark',
      accent: '#5b5bd6',
    })
  })
})

describe('accentVariants', () => {
  it('derives hover and soft variants for a custom colour', () => {
    const variants = accentVariants('#3a7bd5', 'light')
    expect(variants.hover).toBe('#2763b7')
    expect(variants.soft).toBe('#eaeff5')
    // Mid-blue sits above the white-text luminance threshold, so buttons on
    // it take dark text.
    expect(variants.onAccent).toBe('#1a211d')
  })

  it('uses dark text on a pale accent', () => {
    expect(accentVariants('#ffd166', 'light').onAccent).toBe('#1a211d')
  })

  it('lifts a too-dark custom base in dark mode', () => {
    const dark = accentVariants('#111111', 'dark')
    expect(dark.base).not.toBe('#111111')
    expect(dark.soft).toMatch(/^#[0-9a-f]{6}$/)
  })

  it('recognises presets case-insensitively and matches every preset base', () => {
    for (const preset of ACCENT_PRESETS) {
      expect(presetFor(preset.base.toUpperCase())?.id).toBe(preset.id)
      expect(accentVariants(preset.base, 'light').hover).toBe(preset.hoverLight)
      expect(accentVariants(preset.base, 'dark').hover).toBe(preset.hoverDark)
    }
  })
})

describe('initTheme', () => {
  it('paints the cached pair before the first snapshot arrives', async () => {
    fakeMatchMedia(false)
    localStorage.setItem('ledgeling:appearance', JSON.stringify({ theme: 'dark', accent: '#2563d9' }))
    const { initTheme } = await import('./theme')
    initTheme()
    expect(root().dataset.theme).toBe('dark')
    expect(root().style.getPropertyValue('--accent')).toBe('#2563d9')
  })

  it('paints the legacy cached pair if ledgeling:appearance is not set', async () => {
    fakeMatchMedia(false)
    localStorage.removeItem('ledgeling:appearance')
    localStorage.setItem('perch:appearance', JSON.stringify({ theme: 'dark', accent: '#2563d9' }))
    const { applyCachedTheme } = await import('./theme')
    applyCachedTheme()
    expect(root().dataset.theme).toBe('dark')
    expect(root().style.getPropertyValue('--accent')).toBe('#2563d9')
  })

  it('follows the system appearance while the theme is auto', async () => {
    const media = fakeMatchMedia(false)
    const { initTheme } = await import('./theme')
    const state = await import('./state')
    initTheme()
    state.snapshot.value = { settings: { theme: 'system', accent: '#5b5bd6' } } as unknown as Snapshot
    await nextTick()
    expect(root().dataset.theme).toBe('light')
    media.setMatches(true)
    expect(root().dataset.theme).toBe('dark')
    expect(root().style.getPropertyValue('--accent')).toBe('#5b5bd6')
  })
})
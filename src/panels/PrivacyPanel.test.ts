// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import PrivacyPanel from './PrivacyPanel.vue'
import { snapshot } from '../state'
import type { Snapshot } from '../types'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: api.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const baseSettings = {
  version: 1,
  pets: [],
  paused: false,
  hidden: false,
  speedScale: 1,
  debug: false,
  sounds: false,
  launchAtLogin: false,
  awareness: { enabled: true },
  categoryOverrides: {},
  uninstalledBuiltins: [],
  petScreen: null,
  theme: 'system',
  accent: '#31725b',
}

beforeEach(() => {
  api.invoke.mockReset().mockResolvedValue(undefined)
  snapshot.value = { settings: { ...baseSettings } } as unknown as Snapshot
})

describe('appearance preferences', () => {
  it('writes the picked theme through set_settings', async () => {
    const wrapper = mount(PrivacyPanel)
    await wrapper.get('[aria-label="Theme"]').findAll('button')[1].trigger('click')
    expect(api.invoke.mock.calls[0]).toEqual(['set_settings', {
      settings: { ...baseSettings, theme: 'dark' },
    }])
    wrapper.unmount()
  })

  it('writes the picked accent through set_settings', async () => {
    const wrapper = mount(PrivacyPanel)
    await wrapper.get('[aria-label="Accent colour"]').findAll('.swatch')[2].trigger('click')
    expect(api.invoke.mock.calls[0]).toEqual(['set_settings', {
      settings: { ...baseSettings, accent: '#70518f' },
    }])
    wrapper.unmount()
  })

  it('writes a custom accent from the colour picker', async () => {
    const wrapper = mount(PrivacyPanel)
    await wrapper.get('.swatch.custom input[type=color]').setValue('#ff8800')
    expect(api.invoke.mock.calls[0]).toEqual(['set_settings', {
      settings: { ...baseSettings, accent: '#ff8800' },
    }])
    wrapper.unmount()
  })

  it('marks the current theme and accent as checked', () => {
    const wrapper = mount(PrivacyPanel)
    const themeButtons = wrapper.get('[aria-label="Theme"]').findAll('button')
    expect(themeButtons.map(button => button.attributes('aria-checked'))).toEqual([
      'false', 'false', 'true',
    ])
    const swatches = wrapper.get('[aria-label="Accent colour"]').findAll('[role="radio"]')
    expect(swatches[0].attributes('aria-checked')).toBe('true')
    expect(swatches.at(-1)!.attributes('aria-checked')).toBe('false')
    wrapper.unmount()
  })
})
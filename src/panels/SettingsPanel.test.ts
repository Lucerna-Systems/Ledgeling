// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import SettingsPanel from './SettingsPanel.vue'
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
  awareness: { enabled: true, nudge: { enabled: true, tier1Minutes: 45, tier2Minutes: 75, tier3Minutes: 110, cooldownSeconds: 600 } },
  categoryOverrides: {},
  uninstalledBuiltins: [],
  petScreen: null,
  theme: 'system',
  accent: '#5b5bd6',
}

beforeEach(() => {
  api.invoke.mockReset().mockResolvedValue(undefined)
  snapshot.value = { settings: { ...baseSettings } } as unknown as Snapshot
})

it('shows the nearest pace preset and saves the one picked', async () => {
  snapshot.value = { settings: { ...baseSettings, speedScale: 1.3 } } as unknown as Snapshot
  const wrapper = mount(SettingsPanel)
  const buttons = wrapper.get('[aria-label="Pace"]').findAll('button')
  expect(buttons.map(button => button.attributes('aria-checked'))).toEqual(['false', 'true', 'false'])
  await buttons[2].trigger('click')
  expect(api.invoke.mock.calls[0]).toEqual(['set_settings', { settings: { ...baseSettings, speedScale: 1.8 } }])
  wrapper.unmount()
})

describe('appearance preferences', () => {
  it('writes the picked theme through set_settings', async () => {
    const wrapper = mount(SettingsPanel)
    await wrapper.get('[aria-label="Theme"]').findAll('button')[1].trigger('click')
    expect(api.invoke.mock.calls[0]).toEqual(['set_settings', {
      settings: { ...baseSettings, theme: 'dark' },
    }])
    wrapper.unmount()
  })

  it('writes the picked accent through set_settings', async () => {
    const wrapper = mount(SettingsPanel)
    await wrapper.get('[aria-label="Accent colour"]').findAll('.swatch')[2].trigger('click')
    expect(api.invoke.mock.calls[0]).toEqual(['set_settings', {
      settings: { ...baseSettings, accent: '#7c4ddb' },
    }])
    wrapper.unmount()
  })

  it('writes a custom accent from the colour picker', async () => {
    const wrapper = mount(SettingsPanel)
    await wrapper.get('.swatch.custom input[type=color]').setValue('#ff8800')
    expect(api.invoke.mock.calls[0]).toEqual(['set_settings', {
      settings: { ...baseSettings, accent: '#ff8800' },
    }])
    wrapper.unmount()
  })

  it('marks the current theme and accent as checked', () => {
    const wrapper = mount(SettingsPanel)
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

describe('break reminders', () => {
  it('moves the whole ladder when the first reminder changes', async () => {
    const wrapper = mount(SettingsPanel)
    await wrapper.get('[aria-label="Remind me after"]').setValue('60')
    const saved = api.invoke.mock.calls[0][1].settings
    expect(saved.awareness.nudge).toMatchObject({ tier1Minutes: 60, tier2Minutes: 90, tier3Minutes: 125 })
    wrapper.unmount()
  })

  it('offers a custom first reminder alongside the presets', () => {
    snapshot.value!.settings.awareness.nudge.tier1Minutes = 50
    const wrapper = mount(SettingsPanel)
    const options = wrapper.get('[aria-label="Remind me after"]').findAll('option').map(option => option.text())
    expect(options).toEqual(['After 30 min', 'After 45 min', 'After 50 min', 'After 60 min', 'After 90 min'])
    wrapper.unmount()
  })
})

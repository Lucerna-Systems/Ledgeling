import { computed } from 'vue'
import { settings, saveSettings } from '../state'
import type { Settings } from '../types'

export function useSettings() {
  const currentSettings = computed(() => settings.value)
  const isDeveloperMode = computed(() => !!settings.value?.developerMode)
  const isHidden = computed(() => !!settings.value?.hidden)
  const isPaused = computed(() => !!settings.value?.paused)

  async function updateSettings(next: Settings) {
    await saveSettings(next)
  }

  return {
    settings: currentSettings,
    isDeveloperMode,
    isHidden,
    isPaused,
    updateSettings,
  }
}

import { invoke } from '@tauri-apps/api/core'
import { watch } from 'vue'
import logo from './assets/logo.svg?raw'
import { settings } from './state'
import { DEFAULT_ACCENT, accentVariants } from './theme'

const SIZE = 512

/** The product logo repainted from the Iris gradient into the given accent. */
export function tintedLogo(accent: string): string {
  const { base, glow } = accentVariants(accent, 'light')
  return logo.replace('#5b5bd6', base).replace('#8f4fd6', glow)
}

async function rasterise(svg: string): Promise<number[]> {
  const image = new Image()
  image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
  await image.decode()
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = SIZE
  canvas.getContext('2d')!.drawImage(image, 0, 0, SIZE, SIZE)
  const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, 'image/png'))
  if (!blob) throw new Error('Could not encode the app icon')
  return Array.from(new Uint8Array(await blob.arrayBuffer()))
}

/** Keeps the running app's Dock/taskbar icon in the user's accent. Launching
 *  on the default accent leaves the bundled icon alone; returning to it later
 *  sends the default logo, because a dev build has no bundle icon to fall back to. */
export function syncAppIcon(): () => void {
  let applied = DEFAULT_ACCENT
  return watch(() => settings.value?.accent, async accent => {
    if (!accent || accent.toLowerCase() === applied) return
    applied = accent.toLowerCase()
    try {
      const png = await rasterise(tintedLogo(applied))
      // A newer pick may have landed while this one was encoding.
      if (applied === accent.toLowerCase()) await invoke('set_app_icon', { png })
    } catch { /* the bundled icon is a fine fallback */ }
  }, { immediate: true })
}

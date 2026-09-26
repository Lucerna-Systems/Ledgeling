import { expect, it, vi } from 'vitest'
import { tintedLogo } from './appIcon'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

it('repaints both gradient stops of the logo in the chosen accent', () => {
  const svg = tintedLogo('#d2416a')
  expect(svg).toContain('stop-color="#d2416a"')
  expect(svg).toContain('stop-color="#c94aa8"')
  expect(svg).not.toContain('#5b5bd6')
  expect(svg).not.toContain('#8f4fd6')
})

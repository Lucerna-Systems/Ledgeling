export interface Rect { x: number; y: number; width: number; height: number }
export interface Point { x: number; y: number }

export interface Frame {
  petId: number
  clip: string
  frameIndex: number
  x: number
  y: number
  flipX: boolean
  rotation: number
  opacity: number
  width: number
  height: number
  anchorX: number
  anchorY: number
  /** Squash and stretch about the anchor; 1 is the sprite as drawn. */
  squashX: number
  squashY: number
  /** Degrees added to `rotation` while the pet wobbles. */
  lean: number
}

export interface ClipAtlas {
  image: string
  frames: number
  fps: number
  looping: boolean
  pingpong: boolean
  frameWidth: number
  height: number
}

export interface SpawnPayload {
  petId: number
  packId: string
  name: string
  atlas: Record<string, ClipAtlas>
}

export interface DebugOverlay {
  surfaces: [number, number, number, number][]
  jumps: [number, number, number, number][]
  paths: [number, number][][]
  labels: { x: number; y: number; text: string }[]
}

export interface Scene {
  monitor: number
  frames: Frame[]
  props: PropFrame[]
  debug: DebugOverlay | null
}

export type Tier = 'error' | 'warning' | 'info'
export interface Diagnostic { tier: Tier; file: string; path: string; problem: string; expected: string }
export interface Report { diagnostics: Diagnostic[] }

export interface PackSummary {
  id: string
  name: string
  author: string
  version: string
  description: string
  license: string
  tags: string[]
  class: string
  surfaces: string[]
  animations: string[]
  /** Assistant clip slots as this pack resolves them, provided or fallen back. */
  assistantClips: { slot: string; when: string; clip: string; provided: boolean }[]
  preview: string | null
  directory: string
  builtin: boolean
  errors: number
  warnings: number
  /** Info diagnostics: nothing is wrong, but the author may want to know. */
  notes: number
  loadable: boolean
}
export interface PackListing { summary: PackSummary; report: Report }

export type AppCategory = 'code' | 'browser' | 'comms' | 'media' | 'creative' | 'game' | 'other'

export interface Thresholds {
  pausingSeconds: number
  awaySeconds: number
  longAwaySeconds: number
  breakSeconds: number
  microBreakSeconds: number
}
export interface NudgeConfig {
  enabled: boolean
  tier1Minutes: number
  tier2Minutes: number
  tier3Minutes: number
  cooldownSeconds: number
}
export interface AwarenessConfig {
  enabled: boolean
  thresholds: Thresholds
  nudge: NudgeConfig
  categoryDebounce: number
}
export interface PetSetting { packId: string; x: number | null; y: number | null }
export type ThemeMode = 'light' | 'dark' | 'system'
export interface Settings {
  version: number
  pets: PetSetting[]
  paused: boolean
  hidden: boolean
  speedScale: number
  debug: boolean
  sounds: boolean
  launchAtLogin: boolean
  awareness: AwarenessConfig
  categoryOverrides: Record<string, AppCategory>
  uninstalledBuiltins: string[]
  /** Hardware name of the one screen pets live on; null means the main display. */
  petScreen: string | null
  theme: ThemeMode
  /** Accent colour as `#rrggbb`; variants are derived, so only the base travels. */
  accent: string
  /** Developer tools: coding mode, its window and its menu entry. */
  developerMode: boolean
  welcomed: boolean
  /** The "meet your pet" tip card has been dismissed. */
  tipsDismissed: boolean
  effects: Effects
  biomes: BiomeSetting[]
}

/** A scene of decor on one app's frontmost window. */
export interface BiomeSetting {
  id: number
  /** The app's executable name, as the system reports it. */
  app: string
  props: PlacedProp[]
}
export interface PlacedProp { decor: string; prop: string; x: number; flip: boolean }

export interface PropFrame {
  key: string
  decor: string
  prop: string
  frameIndex: number
  x: number
  y: number
  width: number
  height: number
  anchorX: number
  rotation: number
  squashX: number
  squashY: number
  flip: boolean
}
export interface PropAtlas { image: string; frames: number; fps: number; width: number; height: number }
export interface DecorPayload { decor: string; atlas: Record<string, PropAtlas> }

/** The playful physics, each of which can be switched off. */
export interface Effects {
  /** Squash, stretch and wobble. Off, pets still follow windows smoothly. */
  jelly: boolean
  /** Biomes are drawn on their windows. Off, they are kept but not shown. */
  biomes: boolean
}

export interface SessionSummary {
  idleSeconds: number
  idleState: 'active' | 'pausing' | 'away' | 'long_away'
  stretchMinutes: number
  sessionMinutes: number
  microBreaks: number
  breaks: number
}
export interface AwarenessSummary {
  enabled: boolean
  session: SessionSummary
  category: AppCategory
  nudgeTier: number
  energy: number
  mood: number
}

export interface PetView {
  id: number
  packId: string
  name: string
  class: string
  state: string
  clip: string
  motion: { kind: string }
  /** Set while the pet is staying with the user instead of wandering. */
  attend: 'listening' | 'busy' | 'needs_user' | null
  x: number
  y: number
  energy: number
  mood: number
  surface: string | null
  /** Days spent together, and how fond the pet is of the user (0 to 1). */
  days: number
  affection: number
  /** The biome this pet calls home. */
  homeBiome: number | null
}

export interface MonitorInfo { id: number; bounds: Rect; work: Rect; scale: number; isPrimary: boolean }
export interface ScreenView extends MonitorInfo { name: string }
export interface SurfaceView { kind: string; owner: number; x1: number; y1: number; x2: number; y2: number }

export interface Snapshot {
  pets: PetView[]
  packs: PackSummary[]
  settings: Settings
  awareness: AwarenessSummary
  /** The single screen pets live on. */
  monitors: MonitorInfo[]
  /** Every connected screen, for the picker. */
  screens: ScreenView[]
  /** The screen pets are actually on — the user's pick when connected, else main. */
  petScreen: string | null
  windows: { id: number; rect: Rect; zIndex: number; isMinimized: boolean; monitorId: number }[]
  surfaces: SurfaceView[]
  bounds: Rect
  perchCount: number
  scanMs: number
  scanError: string | null
  suspended: boolean
  notes: string[]
  packsDir: string
  native: boolean
  /** False in builds made without coding mode. */
  codingAvailable: boolean
  /** True on a brand-new user's first launch: open on the Pets page once. */
  firstRun: boolean
}

// ---- Contract with the Rust backend: packs, installs and online features ----

/** What `export_pack` returns: where the `.petpack` is, and its SHA-256. */
export interface ExportResult {
  path: string
  sha256: string
}

/**
 * Sent by `install_pack`, `update_pack`, `gallery_install` and (as the network
 * codes) the marketplace. Switch on `code`, never on `message`.
 */
export type InstallErrorCode =
  | 'unreadable_archive' | 'archive_limit' | 'not_allowed' | 'no_manifest'
  | 'invalid_pack' | 'already_installed' | 'not_installed' | 'not_newer'
  | 'bundled' | 'hash_mismatch' | 'io'
  | 'offline' | 'unavailable' | 'refused' | 'failed' | 'too_large' | 'cancelled' | 'http_status'

export interface InstallError {
  code: InstallErrorCode
  message: string
}

export type NetScope = 'marketplace' | 'analytics'
export type NetMode = 'off' | 'session' | 'remembered'

/** One optional online feature, from `net_status`. Everything starts `off`. */
export interface NetScopeStatus {
  scope: NetScope
  label: string
  description: string
  hosts: string[]
  /** This build knows a server for it. When false, hide it: it cannot be switched on. */
  available: boolean
  mode: NetMode
  /** Cannot be granted for one session; only until switched off. */
  alwaysRemembered: boolean
}

/** Where a marketplace-installed pack came from, from `pack_sources`. */
export interface PackSource {
  galleryId: string
  version: string
  sha256: string
}

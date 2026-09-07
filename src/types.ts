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
  preview: string | null
  directory: string
  builtin: boolean
  errors: number
  warnings: number
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
  x: number
  y: number
  energy: number
  mood: number
  surface: string | null
}

export interface MonitorInfo { id: number; bounds: Rect; work: Rect; scale: number; isPrimary: boolean }
export interface SurfaceView { kind: string; owner: number; x1: number; y1: number; x2: number; y2: number }

export interface Snapshot {
  pets: PetView[]
  packs: PackSummary[]
  settings: Settings
  awareness: AwarenessSummary
  monitors: MonitorInfo[]
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
}

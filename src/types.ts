export interface Rect { x: number; y: number; width: number; height: number }
export interface Surface { owner: number; start: number; end: number; y: number; owner_start: number; owner_width: number }
export interface Snapshot {
  activity: 'perched' | 'flying' | 'greeting' | 'held'; paused: boolean; visible: boolean; debug: boolean; speed: number;
  x: number; y: number; clip: 'idle' | 'move'; frameIndex: number; flipX: boolean;
  surfaces: Surface[]; windows: { id: number; rect: Rect }[]; screen: Rect; scale: number;
  scanMs: number; scanError: string | null; renderError: string | null; renderer: string;
}
export interface Frame { clip: 'idle' | 'move'; frameIndex: number; flipX: boolean; x: number; y: number; greeting?: boolean; paused?: boolean }

import type { EditView } from '../types'

/** How tall the strip above a window's top edge is where props can be dropped. */
export const BAND = 120

/** Where along the decorated edge a pointer is, 0 at the left end to 1 at the right. */
export function fractionAlong(clientX: number, edit: Pick<EditView, 'edgeX' | 'edgeWidth'>): number {
  if (edit.edgeWidth <= 0) return 0.5
  return Math.min(1, Math.max(0, (clientX - edit.edgeX) / edit.edgeWidth))
}

/** Is the pointer over the drop strip just above the edge (or a little below it)? */
export function inBand(clientX: number, clientY: number, edit: Pick<EditView, 'edgeX' | 'edgeY' | 'edgeWidth'>): boolean {
  return clientX >= edit.edgeX && clientX <= edit.edgeX + edit.edgeWidth
    && clientY >= edit.edgeY - BAND && clientY <= edit.edgeY + 16
}

/** A palette thumbnail's scale: the prop fitted inside a square of `box` pixels. */
export function thumbScale(width: number, height: number, box = 36): number {
  return Math.min(box / width, box / height, 2)
}

/** Small picture URLs for frame strips and lists. Thumbnails read their canvas
 *  back with toDataURL, which always sees the latest pixels; the cache is keyed
 *  on the pixel buffer and a version the caller bumps after every edit. */
export function thumbnailer() {
  const canvas = document.createElement('canvas')
  const cache = new Map<Uint8ClampedArray, { version: number; url: string }>()
  return (pixels: Uint8ClampedArray | undefined, width: number, height: number, version: number): string => {
    if (!pixels || pixels.length !== width * height * 4) return ''
    const cached = cache.get(pixels)
    if (cached?.version === version) return cached.url
    canvas.width = width
    canvas.height = height
    canvas.getContext('2d')?.putImageData(new ImageData(new Uint8ClampedArray(pixels), width, height), 0, 0)
    const url = pixels.some((v, i) => i % 4 === 3 && v > 0) ? canvas.toDataURL() : ''
    cache.set(pixels, { version, url })
    return url
  }
}

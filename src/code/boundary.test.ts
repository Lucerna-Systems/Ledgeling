import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

/** Coding mode and terminal mode are pages of their own and must cost the pet
 *  and the dashboard nothing: no page outside `src/code` and `src/terminal` may
 *  import them, or the editor and xterm they bring. */
const src = join(__dirname, '..')

function files(dir: string): string[] {
  return readdirSync(dir).flatMap(name => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return path === join(src, 'code') || path === join(src, 'terminal') ? [] : files(path)
    return /\.(ts|vue)$/.test(name) && !name.endsWith('.test.ts') ? [path] : []
  })
}

describe('the coding mode boundary', () => {
  it('keeps Monaco, xterm, coding mode and terminal mode out of the pet and dashboard pages', () => {
    const offenders = files(src).filter(path => {
      // The bubble may fetch its terminal on demand, the one way in.
      const text = readFileSync(path, 'utf8').replace("import('../terminal/BubbleTerminal.vue')", '')
      return /from ['"](monaco-editor|@xterm\/)|from ['"][./]*(code|terminal)\/|import\(['"][./]*(code|terminal)\//.test(text)
    })
    expect(offenders.map(path => relative(src, path))).toEqual([])
  })

  it('loads each coding panel only when its tab is first opened', () => {
    const shell = readFileSync(join(src, 'code', 'CodeApp.vue'), 'utf8')
    expect(shell).not.toMatch(/from ['"]\.\/panels\//)
    expect(shell.match(/import\('\.\/panels\//g)?.length).toBe(8)
  })
})

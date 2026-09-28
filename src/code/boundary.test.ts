import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

/** Coding mode must cost ordinary users nothing: no page outside `src/code`
 *  may import it or the editor and terminal it brings. */
const src = join(__dirname, '..')

function files(dir: string): string[] {
  return readdirSync(dir).flatMap(name => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return path === join(src, 'code') ? [] : files(path)
    return /\.(ts|vue)$/.test(name) && !name.endsWith('.test.ts') ? [path] : []
  })
}

describe('the coding mode boundary', () => {
  it('keeps Monaco, xterm and coding mode out of the pet and dashboard pages', () => {
    const offenders = files(src).filter(path => {
      const text = readFileSync(path, 'utf8')
      return /from ['"](monaco-editor|@xterm\/)|from ['"][./]*code\/|import\(['"][./]*code\//.test(text)
    })
    expect(offenders.map(path => relative(src, path))).toEqual([])
  })

  it('loads each coding panel only when its tab is first opened', () => {
    const shell = readFileSync(join(src, 'code', 'CodeApp.vue'), 'utf8')
    expect(shell).not.toMatch(/from ['"]\.\/panels\//)
    expect(shell.match(/import\('\.\/panels\//g)?.length).toBe(8)
  })
})

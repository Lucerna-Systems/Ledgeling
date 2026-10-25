import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, it } from 'vitest'
import { parse, pick } from './with-env.mjs'

const script = fileURLToPath(new URL('./with-env.mjs', import.meta.url))

it('reads KEY=value lines, comments, quotes and exports', () => {
  const parsed = parse(
    ['# a comment', '', 'A=1', 'B = "two words"', "C='three'", 'export D=4 # inline', 'E=', 'not a line'].join('\r\n'),
  )
  expect(parsed).toEqual({ A: '1', B: 'two words', C: 'three', D: '4', E: '' })
})

it('adds only non-empty variables the shell has not already set', () => {
  expect(pick({ A: '1', B: '', C: '3' }, { C: 'shell' })).toEqual({ A: '1' })
})

it('runs a command with the file added, the shell winning, and never prints a value', () => {
  const dir = mkdtempSync(join(tmpdir(), 'with-env-'))
  const file = join(dir, '.env')
  writeFileSync(file, 'WITH_ENV_NEW=from-file\nWITH_ENV_SHELL=from-file\nWITH_ENV_SECRET=hunter2\n')
  const run = spawnSync(
    process.execPath,
    [script, process.execPath, '-e', 'console.log(process.env.WITH_ENV_NEW + ":" + process.env.WITH_ENV_SHELL)'],
    { env: { ...process.env, LEDGELING_ENV_FILE: file, WITH_ENV_SHELL: 'from-shell' }, encoding: 'utf8' },
  )
  rmSync(dir, { recursive: true })
  expect(run.stdout.trim()).toBe('from-file:from-shell')
  expect(run.stderr).toContain('WITH_ENV_NEW')
  expect(run.stderr).not.toContain('hunter2')
  expect(run.stderr).not.toContain('from-file')
})

it('passes the command exit code through and explains a missing file', () => {
  const run = spawnSync(process.execPath, [script, process.execPath, '-e', 'process.exit(3)'], {
    env: { ...process.env, LEDGELING_ENV_FILE: join(tmpdir(), 'does-not-exist.env') },
    encoding: 'utf8',
  })
  expect(run.status).toBe(3)
  expect(run.stderr).toContain('No .env file')
})

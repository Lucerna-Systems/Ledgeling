import { expect, it } from 'vitest'
import { changedSince, classify, decide } from './ci-changes.mjs'

const needs = files => {
  const { rust, frontend } = classify(files)
  return { rust, frontend }
}

it('sends Rust, pack and schema changes to the matrix', () => {
  expect(needs(['src-tauri/src/net/client.rs'])).toEqual({ rust: true, frontend: false })
  expect(needs(['packs/cat-tabby/pet.json'])).toEqual({ rust: true, frontend: false })
  expect(needs(['schema/pet-pack-v1.schema.json'])).toEqual({ rust: true, frontend: false })
  expect(needs(['sidecar/package.json'])).toEqual({ rust: true, frontend: false })
})

it('sends Vue changes to the frontend tests only', () => {
  expect(needs(['src/panels/PacksPanel.vue'])).toEqual({ rust: false, frontend: true })
  expect(needs(['vite.config.ts'])).toEqual({ rust: false, frontend: true })
  expect(needs(['scripts/with-env.test.mjs'])).toEqual({ rust: false, frontend: true })
})

it('runs nothing for documentation and housekeeping', () => {
  expect(needs(['docs/development.md', 'README.md', 'CLAUDE.md', '.env.example', '.github/workflows/release.yml']))
    .toEqual({ rust: false, frontend: false })
})

it('runs both when a change affects how everything builds, and for anything unknown', () => {
  expect(needs(['package-lock.json'])).toEqual({ rust: true, frontend: true })
  expect(needs(['.github/workflows/verify.yml'])).toEqual({ rust: true, frontend: true })
  expect(needs(['server/main.go'])).toEqual({ rust: true, frontend: true })
  expect(needs(['docs/a.md', 'src/App.vue'])).toEqual({ rust: false, frontend: true })
  expect(needs(['src/App.vue', 'src-tauri/src/app.rs'])).toEqual({ rust: true, frontend: true })
})

it('runs everything when there is no earlier good run or it is not in this history', () => {
  expect(decide('', null)).toMatchObject({ rust: true, frontend: true })
  expect(decide('abc1234def', null)).toMatchObject({ rust: true, frontend: true })
  expect(changedSince('')).toBeNull()
  expect(changedSince('abc', () => { throw new Error('bad object') })).toBeNull()
})

it('runs nothing when nothing changed since the last good run', () => {
  expect(decide('abc1234', [])).toMatchObject({ rust: false, frontend: false })
  expect(changedSince('abc', args => (args[0] === 'diff' ? 'src/a.vue\n\ndocs/b.md\n' : ''))).toEqual(['src/a.vue', 'docs/b.md'])
})

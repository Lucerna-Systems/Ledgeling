import { describe, expect, it } from 'vitest'
import { ago, issueBranch, linkRefs, localTarget, parseDiff, prState, rollup, runId, shellQuote } from './format'
import type { Check } from './types'

const run = (status: string, conclusion = ''): Check => ({ __typename: 'CheckRun', name: 'x', status, conclusion })

describe('the GitHub view helpers', () => {
  it('splits a unified diff into files with line numbers', () => {
    const files = parseDiff([
      'diff --git a/src/a.ts b/src/a.ts',
      'index 1..2 100644',
      '--- a/src/a.ts',
      '+++ b/src/a.ts',
      '@@ -10,3 +10,3 @@ function f() {',
      ' keep',
      '-old',
      '+new',
      '\\ No newline at end of file',
      'diff --git a/new.txt b/new.txt',
      'new file mode 100644',
      '@@ -0,0 +1 @@',
      '+hello',
      'diff --git a/old.md b/renamed.md',
      'similarity index 100%',
      'rename from old.md',
      'rename to renamed.md',
    ].join('\n'))
    expect(files.map(f => [f.path, f.status, f.added, f.removed])).toEqual([
      ['src/a.ts', 'modified', 1, 1], ['new.txt', 'added', 1, 0], ['renamed.md', 'renamed', 0, 0],
    ])
    expect(files[0].hunks[0].header).toBe('function f() {')
    expect(files[0].hunks[0].lines.map(l => [l.kind, l.old, l.new])).toEqual([
      ['ctx', 10, 10], ['del', 11, null], ['add', null, 11], ['note', null, null],
    ])
    expect(files[2].oldPath).toBe('old.md')
  })

  it('rolls checks up the way GitHub shows them', () => {
    expect(rollup([]).state).toBe('none')
    expect(rollup([run('COMPLETED', 'SUCCESS'), run('COMPLETED', 'SKIPPED')])).toMatchObject({ state: 'pass', total: 1 })
    expect(rollup([run('COMPLETED', 'SUCCESS'), run('IN_PROGRESS')]).state).toBe('pending')
    expect(rollup([run('IN_PROGRESS'), run('COMPLETED', 'FAILURE')]).state).toBe('fail')
    expect(rollup([{ __typename: 'StatusContext', context: 'ci', state: 'SUCCESS' }]).state).toBe('pass')
  })

  it('opens links to this repository here and everything else elsewhere', () => {
    const repo = 'https://github.com/Octo/App'
    expect(localTarget('https://github.com/octo/app/pull/12', repo)).toEqual({ kind: 'pr', number: 12 })
    expect(localTarget('https://github.com/Octo/App/issues/3#issuecomment-1', repo)).toEqual({ kind: 'issue', number: 3 })
    expect(localTarget('https://github.com/other/app/issues/3', repo)).toBeNull()
    expect(localTarget('https://github.com/Octo/App/pulls', repo)).toBeNull()
  })

  it('links #numbers in prose but not in code', () => {
    const out = linkRefs('Fixes #4, see `#5` and\n```\n#6\n```', 'https://github.com/o/r')
    expect(out).toContain('[#4](https://github.com/o/r/issues/4)')
    expect(out).toContain('`#5`')
    expect(out).toContain('\n#6\n')
  })

  it('names branches, states, runs and quoting predictably', () => {
    expect(issueBranch(12, 'Fix the “weird” crash on start-up!')).toBe('issue-12-fix-the-weird-crash-on-start')
    expect(issueBranch(3, '???')).toBe('issue-3')
    expect(prState({ state: 'OPEN', isDraft: true })).toBe('draft')
    expect(prState({ state: 'MERGED', isDraft: false })).toBe('merged')
    expect(runId('https://github.com/o/r/actions/runs/36335407500/job/1')).toBe(36335407500)
    expect(runId(null)).toBeNull()
    expect(shellQuote("it's", false)).toBe(`'it'\\''s'`)
    expect(shellQuote('say "hi"', true)).toBe('"say \\"hi\\""')
    expect(ago(new Date(Date.now() - 3 * 3600_000).toISOString())).toBe('3h ago')
  })
})

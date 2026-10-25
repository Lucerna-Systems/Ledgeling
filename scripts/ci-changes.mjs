// Decides which parts of CI a change needs, from the files that changed since
// the last release run that succeeded (the last known-good state).
//
//   node scripts/ci-changes.mjs <base-sha>      writes rust= and frontend= to
//                                               $GITHUB_OUTPUT and a summary
//
// Anything it does not recognise runs everything, so a new folder can only
// make CI slower, never skip a test it should have run.
import { execFileSync } from 'node:child_process'
import { appendFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { resolve } from 'node:path'

/** The Rust matrix: the app, its bundled packs and schema, and its build inputs. */
const RUST_PREFIXES = ['src-tauri/', 'packs/', 'schema/', 'sidecar/']
const RUST_FILES = ['scripts/linux-display-tests.sh', 'scripts/prepare-sidecar.mjs']

/** The frontend: type-check, build and unit tests. */
const FRONTEND_PREFIXES = ['src/', 'scripts/']
const FRONTEND_FILES = ['index.html', 'code.html', 'terminal.html', 'vite.config.ts', 'tsconfig.json']

/** Both: these change how everything is built or tested. */
const BOTH_FILES = ['package.json', 'package-lock.json', '.github/workflows/verify.yml']

/** Nothing to test: documentation and repository housekeeping. */
const IGNORED_PREFIXES = ['docs/', 'planning_files/']
const IGNORED_FILES = ['LICENSE.txt', '.gitignore', '.env.example', 'AGENTS.md', 'CLAUDE.md', 'README.md', '.github/workflows/release.yml']

const within = (file, prefixes) => prefixes.some(p => file.startsWith(p))

/** Which jobs a list of changed files needs. Unknown files need both. */
export function classify(files) {
  let rust = false
  let frontend = false
  const reasons = []
  for (const file of files) {
    if (BOTH_FILES.includes(file)) {
      rust = frontend = true
      reasons.push(`${file}: both`)
    } else if (RUST_FILES.includes(file) || within(file, RUST_PREFIXES)) {
      rust = true
      reasons.push(`${file}: rust`)
    } else if (FRONTEND_FILES.includes(file) || within(file, FRONTEND_PREFIXES)) {
      frontend = true
      reasons.push(`${file}: frontend`)
    } else if (IGNORED_FILES.includes(file) || within(file, IGNORED_PREFIXES) || file.endsWith('.md')) {
      reasons.push(`${file}: ignored`)
    } else {
      rust = frontend = true
      reasons.push(`${file}: unrecognised, so both`)
    }
  }
  return { rust, frontend, reasons }
}

/** The changed files since `base`, or `null` when that cannot be worked out. */
export function changedSince(base, git = args => execFileSync('git', args, { encoding: 'utf8' })) {
  if (!base) return null
  try {
    git(['cat-file', '-e', `${base}^{commit}`])
    return git(['diff', '--name-only', base, 'HEAD']).split('\n').filter(Boolean)
  } catch {
    return null
  }
}

/** The decision for CI, given the last good commit (empty when there is none). */
export function decide(base, files) {
  if (files === null) {
    const why = base ? `${base.slice(0, 7)} is not in this history` : 'no earlier successful run'
    return { rust: true, frontend: true, reasons: [`${why}, so everything runs`] }
  }
  if (files.length === 0) return { rust: false, frontend: false, reasons: ['nothing changed since the last good run'] }
  return classify(files)
}

function main() {
  const base = process.argv[2] || ''
  const result = decide(base, changedSince(base))
  const lines = [
    `Compared with ${base ? base.slice(0, 7) : 'nothing'}: rust=${result.rust} frontend=${result.frontend}`,
    ...result.reasons.map(r => `  ${r}`),
  ]
  console.log(lines.join('\n'))
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(process.env.GITHUB_OUTPUT, `rust=${result.rust}\nfrontend=${result.frontend}\n`)
  }
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### What changed\n\n\`\`\`\n${lines.join('\n')}\n\`\`\`\n`)
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()

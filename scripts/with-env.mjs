// Runs a command with the variables from `.env` added to its environment, so
// one file feeds `tauri dev`, `tauri build` and any script, on every platform.
//
//   node scripts/with-env.mjs <command> [args...]
//
// Variables already set in the shell win over the file. `LEDGELING_ENV_FILE`
// names a different file, for a worktree that shares one `.env`. Only variable
// names are ever printed, never values.
import { spawn } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(fileURLToPath(new URL('..', import.meta.url)))
const file = resolve(process.env.LEDGELING_ENV_FILE || resolve(root, '.env'))

/** `KEY=value` lines; `#` comments, blank lines and optional quotes. */
export function parse(text) {
  const out = {}
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    const match = /^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/.exec(line)
    if (!match) continue
    let value = match[2].trim()
    const quote = value[0]
    if ((quote === '"' || quote === "'") && value.endsWith(quote) && value.length >= 2) {
      value = value.slice(1, -1)
    } else {
      // An unquoted value ends at an inline comment.
      value = value.replace(/\s+#.*$/, '')
    }
    out[match[1]] = value
  }
  return out
}

/** The file's variables that the shell has not already set, and that are not empty. */
export function pick(parsed, env) {
  return Object.fromEntries(
    Object.entries(parsed).filter(([key, value]) => value !== '' && env[key] === undefined),
  )
}

function main() {
  const [command, ...args] = process.argv.slice(2)
  if (!command) {
    console.error('Usage: node scripts/with-env.mjs <command> [args...]')
    process.exit(2)
  }
  let added = {}
  if (existsSync(file)) {
    added = pick(parse(readFileSync(file, 'utf8')), process.env)
    const names = Object.keys(added)
    console.error(names.length ? `.env: ${names.join(', ')}` : '.env: nothing to add')
  } else {
    console.error('No .env file; copy .env.example to .env to set variables.')
  }
  const child = spawn(command, args, {
    stdio: 'inherit',
    env: { ...process.env, ...added },
    // npm and npx are .cmd shims on Windows, which only a shell can start.
    shell: process.platform === 'win32',
  })
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal))
  child.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)))
  child.on('error', error => {
    console.error(`Could not run ${command}: ${error.message}`)
    process.exit(127)
  })
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main()

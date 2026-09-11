import { copyFile, chmod, access } from 'node:fs/promises'
import { resolve } from 'node:path'
const root = resolve(import.meta.dirname, '..')
await access(resolve(root, 'sidecar/node_modules/@anthropic-ai/claude-agent-sdk/sdk.mjs'))
const destination = resolve(root, 'sidecar', process.platform === 'win32' ? 'node.exe' : 'node')
await copyFile(process.execPath, destination)
await chmod(destination, 0o755)
console.log(`Prepared Claude bridge for ${process.platform}/${process.arch}`)

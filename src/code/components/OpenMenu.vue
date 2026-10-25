<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { AppWindow, ChevronDown, Globe, Hammer, Package, Play, SquareArrowOutUpRight } from '@lucide/vue'
import { code } from '../state'
import { appsOf, live, portsOf, refreshLive, urlFor } from '../live'
import { nextTitle, spawnPane } from '../../terminal/panes'
import { terms } from '../../terminal/terminals'
import type { AppView, NativeApp, RunConfig, RunSetup } from '../types'

/** The open button on a terminal: what that terminal is serving, one click
 *  away, and how to start or preview the project when it serves nothing yet. */
const props = defineProps<{ session: number; worktree: string }>()

const open = ref(false)
const button = ref<HTMLElement | null>(null)
const place = ref({ top: 0, right: 0 })
const native = ref<NativeApp | null>(null)
const setup = ref<RunSetup | null>(null)

const ports = computed(() => portsOf(props.session))
const apps = computed(() => appsOf(props.session))
/** Desktop apps running from this worktree's other terminals. */
const elsewhere = computed(() => live.value.apps.filter(a => a.session !== props.session && terms.get(a.session)?.worktree === props.worktree))

/** A running desktop app wins over its dev server's port: the app is what
 *  the port is for. */
const primary = computed(() => {
  const app = apps.value[0]
  if (app) return { label: app.name || 'App', title: `Bring ${app.name || 'the app'} forward`, icon: AppWindow, run: () => activate(app) }
  const port = ports.value[0]
  if (port) return { label: `:${port.port}`, title: `Open ${urlFor(port.port)} in your browser`, icon: Globe, run: () => browse(port.port) }
  return null
})

async function toggle() {
  if (open.value) { open.value = false; return }
  const rect = button.value?.getBoundingClientRect()
  if (rect) place.value = { top: rect.bottom + 4, right: Math.max(8, window.innerWidth - rect.right) }
  open.value = true
  void refreshLive()
  // Loaded on every open so a build that just finished shows up.
  const [app, run] = await Promise.all([
    code<NativeApp | null>('code_launch', { worktree: props.worktree }).catch(() => null),
    code<RunSetup>('code_run_setup', { worktree: props.worktree }).catch(() => null),
  ])
  native.value = app
  setup.value = run
}

async function activate(app: AppView) {
  open.value = false
  await code('code_activate', { pid: app.pid }).catch(() => {})
}
async function browse(port: number) {
  open.value = false
  await code('code_open_url', { url: urlFor(port) }).catch(() => {})
}
async function openBuild() {
  open.value = false
  await code('code_launch_open', { worktree: props.worktree }).catch(() => {})
}
function inPane(command: string, title: string, dir: 'row' | 'column' = 'row') {
  open.value = false
  void spawnPane({ worktree: props.worktree, kind: 'shell', command, title: nextTitle(props.worktree, title) }, props.session, dir)
}

const runs = computed(() => [...terms.values()].filter(t => t.kind === 'run' && t.alive && t.worktree === props.worktree))
const runningAs = (config: RunConfig) => runs.value.find(t => t.title === config.name) ?? null
function startRun(config: RunConfig, index: number) {
  open.value = false
  const running = runningAs(config)
  const port = running && portsOf(running.id)[0]?.port
  if (running && (config.url || port)) { void code('code_open_url', { url: config.url ?? urlFor(port as number) }).catch(() => {}); return }
  if (running) return
  void spawnPane({ worktree: props.worktree, kind: 'run', title: config.name, runIndex: index }, props.session)
}

function ago(ms: number) {
  const minutes = Math.round((Date.now() - ms) / 60000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes} min ago`
  const hours = Math.round(minutes / 60)
  if (hours < 48) return `${hours} h ago`
  return `${Math.round(hours / 24)} days ago`
}

const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') open.value = false }
onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <span ref="button" class="open-split" :class="{ lit: primary }">
    <button v-if="primary" class="open-main" :title="primary.title" @click.stop="primary.run()">
      <component :is="primary.icon" :size="12" /> {{ primary.label }}
    </button>
    <button v-else class="open-main quiet" title="Open what this terminal is serving" @click.stop="toggle">
      <SquareArrowOutUpRight :size="12" /> Open
    </button>
    <button class="open-chevron" :aria-expanded="open" aria-haspopup="menu" title="More ways to open" @click.stop="toggle"><ChevronDown :size="12" /></button>
  </span>
  <Teleport to="body">
    <template v-if="open">
      <div class="popover-scrim" @pointerdown="open = false" />
      <div class="open-menu" role="menu" :style="{ top: `${place.top}px`, right: `${place.right}px`, maxHeight: `calc(100vh - ${place.top + 8}px)` }">
        <div class="menu-group">This terminal</div>
        <button v-for="app in apps" :key="app.pid" role="menuitem" @click="activate(app)">
          <AppWindow :size="14" /><span><strong>Bring {{ app.name || 'the app' }} forward</strong><small>Running from this terminal</small></span>
        </button>
        <button v-for="port in ports" :key="port.port" role="menuitem" @click="browse(port.port)">
          <Globe :size="14" /><span><strong>Open localhost:{{ port.port }}</strong><small>{{ port.process || 'Listening' }} · in your browser</small></span>
        </button>
        <p v-if="!apps.length && !ports.length" class="menu-note">Nothing is open yet. Start a dev server here and its address appears on this button.</p>

        <template v-if="native">
          <div class="menu-group">{{ native.framework }} desktop app</div>
          <button v-for="app in elsewhere" :key="app.pid" role="menuitem" @click="activate(app)">
            <AppWindow :size="14" /><span><strong>Show the running dev app</strong><small>{{ app.name }} · from {{ terms.get(app.session)?.title }}</small></span>
          </button>
          <button v-if="!apps.length && !elsewhere.length" role="menuitem" @click="inPane(native.dev, 'Dev app')">
            <Play :size="14" /><span><strong>Start the dev app</strong><small class="mono">{{ native.dev }}</small></span>
          </button>
          <button role="menuitem" :disabled="!native.built" @click="openBuild">
            <Package :size="14" /><span><strong>Open the production build</strong><small>{{ native.built ? `${native.built.name} · built ${ago(native.built.modified)}` : 'Not built yet' }}</small></span>
          </button>
          <button role="menuitem" @click="inPane(native.build, 'Preview build', 'column')">
            <Hammer :size="14" /><span><strong>{{ native.built ? 'Rebuild for preview' : 'Build for preview' }}</strong><small class="mono">{{ native.build }}</small></span>
          </button>
        </template>

        <template v-if="setup?.configs.length">
          <div class="menu-group">Run</div>
          <button v-for="(config, index) in setup.configs" :key="config.name" role="menuitem" @click="startRun(config, index)">
            <component :is="runningAs(config) ? Globe : Play" :size="14" />
            <span><strong>{{ runningAs(config) ? `Open ${config.name}` : `Start ${config.name}` }}</strong><small class="mono">{{ runningAs(config) ? (config.url ?? 'Running') : config.command }}</small></span>
          </button>
        </template>
      </div>
    </template>
  </Teleport>
</template>

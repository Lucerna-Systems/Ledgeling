<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Bot, ExternalLink, Globe, MessageSquare, Pencil, Play, RotateCw, Save, Square, Wand2 } from '@lucide/vue'
import TermView from '../components/TermView.vue'
import { adopt, close, spawn, terms } from '../terminals'
import { pendingTerminal } from '../terminalFocus'
import { chatDraft } from '../chatDraft'
import { code, codeState, current, fileChanges, fileTick, refreshState, root, tab, wide } from '../state'
import { live as liveState, refreshLive, useLive } from '../live'
import { has, loadTools } from '../tools'
import type { RunConfig, RunSetup } from '../types'

const props = defineProps<{ active?: boolean }>()
const setup = ref<RunSetup | null>(null)
const ports = computed(() => liveState.value.ports)
const editing = ref<RunConfig | null>(null)
const showAll = ref(false)
const shownLog = ref<number | null>(null)

async function load() {
  if (!root.value) return
  try { setup.value = await code<RunSetup>('code_run_setup', { worktree: root.value }) } catch { setup.value = null }
}
watch(root, () => { setup.value = null; editing.value = null; void load() }, { immediate: true })
// An agent writing .ledgeling/run.json shows up here without a refresh.
watch(fileTick, () => { if (fileChanges.value.paths.some(p => p.startsWith('.ledgeling/'))) void load() })

/** Running entries for this worktree, by config name. */
const runs = computed(() => [...terms.values()].filter(t => t.kind === 'run' && t.worktree === root.value))
const runFor = (config: RunConfig) => runs.value.filter(t => t.title === config.name).at(-1) ?? null
const live = computed(() => runs.value.some(t => t.alive))
const log = computed(() => runs.value.find(t => t.id === shownLog.value) ?? runs.value.at(-1) ?? null)

async function run(index: number) {
  if (!root.value || !setup.value) return
  const config = setup.value.configs[index]
  const previous = runFor(config)
  if (previous) await close(previous.id)
  const entry = await spawn({ worktree: root.value, kind: 'run', title: config.name, runIndex: index })
  if (entry) shownLog.value = entry.id
  void refreshState()
  void refreshLive()
}
async function stop(config: RunConfig) {
  const entry = runFor(config)
  if (entry) await close(entry.id)
  void refreshState()
}

async function save(configs: RunConfig[]) {
  if (!root.value) return
  try { await code('code_run_save', { worktree: root.value, configs }); editing.value = null; await load() } catch { /* banner */ }
}
function edit(config?: RunConfig) {
  editing.value = config ? { ...config } : { name: 'App', command: '', cwd: '.', port: null, url: null, env: {} }
}
function saveEdit() {
  const draft = editing.value
  if (!draft || !setup.value) return
  const port = draft.port ? Number(draft.port) : null
  const next = { ...draft, port, url: draft.url || (port ? `http://localhost:${port}` : null) }
  const existing = setup.value.fromFile ? setup.value.configs : []
  const at = existing.findIndex(c => c.name === draft.name)
  void save(at >= 0 ? existing.map((c, i) => i === at ? next : c) : [...existing, next])
}

const windows = navigator.userAgent.includes('Windows')
/** Quoted for the shell the terminal runs commands through. */
const quote = (text: string) => windows ? `"${text.replace(/"/g, '\\"')}"` : `'${text.replace(/'/g, `'\\''`)}'`

async function askAI(via: 'claude' | 'chat') {
  const prompt = await code<string>('code_run_prompt').catch(() => '')
  if (!prompt || !root.value) return
  if (via === 'claude') {
    // Interactive, so Claude asks before it writes the file and you can watch.
    pendingTerminal.value = { worktree: root.value, kind: 'agent', title: 'Claude Code', command: `claude ${quote(prompt)}` }
    tab.value = 'terminal'
  } else {
    chatDraft.value = prompt
    tab.value = 'chat'
  }
}

async function open(url: string | null) { if (url) await code('code_open_url', { url }).catch(() => {}) }

/** Only while you are looking or something of yours is running. */
useLive(() => !!props.active || live.value)
onMounted(() => { void adopt(); void loadTools() })

const mine = computed(() => ports.value.filter(p => p.session !== null && runs.value.some(r => r.id === p.session)))
const listening = (config: RunConfig) => {
  const entry = runFor(config)
  if (!entry?.alive) return null
  const owned = ports.value.filter(p => p.session === entry.id)
  return owned.find(p => p.port === config.port) ?? owned[0] ?? null
}
const urlFor = (config: RunConfig) => config.url ?? (listening(config) ? `http://localhost:${listening(config)!.port}` : null)
const visiblePorts = computed(() => showAll.value ? ports.value : ports.value.filter(p => p.session !== null))
const sessionTitle = (id: number | null) => id == null ? '' : codeState.value.sessions.find(s => s.id === id)?.title ?? ''
</script>

<template>
  <section>
    <div class="panel-bar">
      <h2 class="grow">Run {{ current?.worktree.name }}</h2>
      <button v-if="setup?.fromFile && !editing" class="icon" title="Add another thing to run" @click="edit()"><Pencil :size="13" /></button>
      <button class="icon" title="Check again" @click="load(); refreshLive()"><RotateCw :size="13" /></button>
    </div>
    <div class="panel-scroll">
      <div v-if="setup?.problems.length" class="banner error" role="alert">
        <p><strong>.ledgeling/run.json needs fixing.</strong><br><span v-for="p in setup.problems" :key="p.path + p.problem">{{ p.path || 'file' }}: {{ p.problem }} Expected {{ p.expected }}.<br></span></p>
      </div>

      <div v-if="setup?.configs.length" class="cards" style="padding-top:10px">
        <article v-for="(config, index) in setup.configs" :key="config.name" class="card">
          <div class="card-head">
            <div class="grow">
              <strong>{{ config.name }}</strong>
              <small class="mono">{{ config.command }}{{ config.cwd !== '.' ? ` · in ${config.cwd}` : '' }}</small>
            </div>
            <span v-if="listening(config)" class="chip live"><span class="dot live" /> :{{ listening(config)!.port }}</span>
            <span v-else-if="runFor(config)?.alive" class="chip busy"><span class="dot busy" /> Starting</span>
            <span v-else-if="runFor(config) && !runFor(config)!.alive" class="chip">Stopped</span>
          </div>
          <div class="card-actions">
            <button v-if="!runFor(config)?.alive" class="primary-button" @click="run(index)"><Play :size="13" /> Run</button>
            <template v-else>
              <button @click="stop(config)"><Square :size="12" /> Stop</button>
              <button class="ghost" @click="run(index)"><RotateCw :size="13" /> Restart</button>
            </template>
            <button v-if="urlFor(config)" class="ghost" :disabled="!listening(config)" :title="listening(config) ? urlFor(config)! : 'Opens once the server is listening'" @click="open(urlFor(config))"><Globe :size="13" /> Open in browser</button>
            <button class="ghost" title="Edit" @click="edit(config)"><Pencil :size="12" /></button>
          </div>
        </article>
      </div>

      <template v-else-if="setup && !editing">
        <div class="empty" style="height:auto;padding-top:22px">
          <span class="glyph"><Play :size="22" /></span>
          <h2>How does this project run?</h2>
          <p>Pick a guess, write it yourself, or let an AI look at the project and fill it in. It is saved in <code>.ledgeling/run.json</code>.</p>
        </div>
        <div class="cards">
          <article v-for="s in setup.suggestions" :key="s.config.command" class="card">
            <div class="card-head">
              <div class="grow"><strong class="mono">{{ s.config.command }}</strong><small>Because {{ s.reason }}{{ s.config.port ? ` · port ${s.config.port}` : '' }}</small></div>
              <button class="primary-button" @click="save([s.config])">Use this</button>
            </div>
          </article>
          <article class="card">
            <div class="card-head"><div class="grow"><strong><Wand2 :size="13" /> Ask AI to set this up</strong><small>It reads the project and writes the run file for you.</small></div></div>
            <div class="card-actions">
              <button :disabled="!has('claude')" :title="has('claude') ? '' : 'Install Claude Code from Add-ons first'" @click="askAI('claude')"><Bot :size="13" /> With Claude Code</button>
              <button @click="askAI('chat')"><MessageSquare :size="13" /> With your chat model</button>
              <button class="ghost" @click="edit()"><Pencil :size="12" /> Write it myself</button>
            </div>
          </article>
        </div>
      </template>

      <div v-if="editing" class="cards" style="padding-top:10px">
        <form class="card form" @submit.prevent="saveEdit">
          <div class="pair">
            <label>Name<input v-model="editing.name" v-focus required placeholder="Web app"></label>
            <label>Port<input v-model.number="editing.port" type="number" min="1" max="65535" placeholder="e.g. 5173"></label>
          </div>
          <label>Command<input v-model="editing.command" class="mono" required placeholder="npm run dev"></label>
          <div class="pair">
            <label>Folder<input v-model="editing.cwd" placeholder="."></label>
            <label>Address<input v-model="editing.url" placeholder="http://localhost:5173"></label>
          </div>
          <div class="card-actions">
            <button class="primary-button" type="submit"><Save :size="13" /> Save</button>
            <button class="ghost" type="button" @click="editing = null">Cancel</button>
          </div>
        </form>
      </div>

      <div class="section-title">
        Ports <span class="muted" style="text-transform:none;letter-spacing:0;font-weight:500">· {{ mine.length ? `${mine.length} started here` : 'none started here' }}</span>
        <span style="flex:1" />
        <label class="small" style="text-transform:none;letter-spacing:0;font-weight:500;display:flex;gap:4px;align-items:center"><input v-model="showAll" type="checkbox"> Show all</label>
      </div>
      <table class="ports">
        <tbody>
          <tr v-for="p in visiblePorts" :key="`${p.port}-${p.pid}`" :class="{ mine: p.session !== null }">
            <td>:{{ p.port }}</td>
            <td>{{ p.process || 'unknown' }} <span class="muted small">pid {{ p.pid }}</span></td>
            <td><span v-if="p.session !== null" class="chip accent">{{ sessionTitle(p.session) || 'Yours' }}</span><span v-else-if="p.exposed" class="chip" title="Reachable from other devices on your network">Network</span></td>
            <td><button class="icon" :title="`Open http://localhost:${p.port}`" @click="open(`http://localhost:${p.port}`)"><ExternalLink :size="13" /></button></td>
          </tr>
          <tr v-if="!visiblePorts.length"><td colspan="4" class="muted small" style="font-family:inherit;font-weight:400">{{ showAll ? 'Nothing is listening.' : 'Nothing you started here is listening yet.' }}</td></tr>
        </tbody>
      </table>
    </div>
    <!-- Side by side with the terminals, each run is a pane in their grid instead. -->
    <div v-if="log && !wide" class="run-log">
      <div class="term-tabs">
        <div v-for="r in runs" :key="r.id" class="term-tab" :class="{ active: r.id === log.id }" @click="shownLog = r.id"><Play :size="11" /> {{ r.title }}<span v-if="!r.alive" class="muted small">· ended</span></div>
      </div>
      <TermView :id="log.id" :key="log.id" :active="!!props.active" :focus="false" />
    </div>
  </section>
</template>

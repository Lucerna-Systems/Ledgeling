<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Bot, Maximize2, Plus, SquareTerminal, X } from '@lucide/vue'
import TermView from './TermView.vue'
import RenameTitle from './RenameTitle.vue'
import AgentChip from './AgentChip.vue'
import ConfirmDialog from './ConfirmDialog.vue'
import { ask } from './confirm'
import { adopt, close, rename, spawn, terms } from './terminals'
import { nextTitle } from './panes'
import { info, pet, problem, refresh, scope, statusOf } from './terminalState'
import type { AgentTool } from './types'
import './base.css'
import './terminal.css'

/** The pet's terminal, inside its right-click bubble: a shell or Claude Code in
 *  the folder the pet works in. The terminals live in Rust, so closing the
 *  bubble does not end them; the focused window shows the same ones. */
const props = defineProps<{ petId: number }>()
const emit = defineEmits<{ popOut: [] }>()

watch(() => props.petId, id => { pet.value = id }, { immediate: true })
const folder = computed(() => scope.value ?? '')
const mine = computed(() => [...terms.values()].filter(t => t.worktree === folder.value).sort((a, b) => a.id - b.id))
const active = ref<number | null>(null)
watch(mine, list => { if (!list.some(t => t.id === active.value)) active.value = list.at(-1)?.id ?? null }, { immediate: true })

/** A shell, or one of the agents installed on this machine, in the pet's folder. */
async function start(agent?: AgentTool) {
  if (!folder.value) return
  const entry = await spawn(agent
    ? { worktree: folder.value, kind: 'agent', command: agent.command, title: nextTitle(folder.value, agent.name) }
    : { worktree: folder.value, kind: 'shell', title: nextTitle(folder.value, 'Shell') })
  if (entry) active.value = entry.id
}

async function closeTab(id: number) {
  const entry = terms.get(id)
  if (entry?.alive && !await ask(`Stop ${entry.title}? Whatever it is running ends too.`, `Stop ${entry.title}`)) return
  await close(id)
}

let starting = false
/** Landing on a blank panel helps nobody: pick up what is already running in
 *  this folder, and start a shell when nothing is. */
async function ensure() {
  if (!folder.value || starting) return
  starting = true
  try {
    await adopt(s => s.worktree === folder.value)
    if (!mine.value.some(t => t.alive)) await start()
  } finally { starting = false }
}

let timer: ReturnType<typeof setInterval> | undefined
onMounted(async () => {
  await refresh()
  watch(folder, () => void ensure(), { immediate: true })
  timer = setInterval(() => { if (!document.hidden) void refresh() }, 1500)
})
onUnmounted(() => clearInterval(timer))
</script>

<template>
  <div class="bubble-term" role="group" aria-label="Terminal">
    <div class="bt-bar">
      <div class="bt-tabs" role="tablist" aria-label="Terminals">
        <div v-for="t in mine" :key="t.id" role="tab" tabindex="0" class="bt-tab" :class="{ ended: !t.alive }" :aria-selected="t.id === active" @click="active = t.id" @keydown.enter.self="active = t.id">
          <Bot v-if="t.kind === 'agent'" :size="12" /><SquareTerminal v-else :size="12" />
          <RenameTitle class="bt-title" :title="t.title" @rename="rename(t.id, $event)" />
          <AgentChip v-if="t.alive && statusOf(t.id)" :status="statusOf(t.id)!" />
          <span class="bt-x" role="button" tabindex="0" :aria-label="`Close ${t.title}`" @click.stop="closeTab(t.id)" @keydown.enter.stop="closeTab(t.id)"><X :size="11" /></span>
        </div>
      </div>
      <button type="button" class="bt-btn" title="New shell" aria-label="New shell" :disabled="!folder" @click="start()"><Plus :size="13" /></button>
      <button type="button" class="bt-btn" title="Open in the focused terminal window" aria-label="Open in the focused terminal window" @click="emit('popOut')"><Maximize2 :size="13" /></button>
    </div>
    <div class="bt-agents" aria-label="Start a coding agent">
      <template v-if="info.agents.length">
        <span class="bt-label">Start</span>
        <button v-for="agent in info.agents" :key="agent.id" type="button" class="bt-agent" :disabled="!folder" :title="`Run ${agent.command} in this folder`" @click="start(agent)"><Bot :size="12" />{{ agent.name }}</button>
      </template>
      <span v-else class="bt-label">No coding agents found on your PATH.</span>
    </div>
    <div class="bt-body">
      <TermView v-if="active !== null" :key="active" :id="active" :active="true" :focus="true" />
      <p v-else class="bt-empty">Starting a terminal…</p>
    </div>
    <p v-if="problem" class="bt-problem" role="alert">{{ problem }} <button type="button" @click="problem = ''">Dismiss</button></p>
    <ConfirmDialog />
  </div>
</template>

<style scoped>
.bubble-term { display: grid; gap: 6px; min-width: 0; }
.bt-bar { display: flex; align-items: center; gap: 4px; min-width: 0; }
.bt-tabs { display: flex; gap: 3px; flex: 1; min-width: 0; overflow-x: auto; scrollbar-width: none; }
.bt-tabs::-webkit-scrollbar { display: none; }
.bt-tab { cursor: pointer; display: inline-flex; align-items: center; gap: 5px; flex-shrink: 0; max-width: 190px; height: 26px; padding: 0 5px 0 8px; border-radius: 8px; font-size: 11.5px; color: var(--muted); }
.bt-tab:hover { background: var(--soft); }
.bt-tab[aria-selected="true"] { background: var(--soft); color: var(--ink); box-shadow: inset 0 0 0 1px var(--line); }
.bt-tab.ended .bt-title { text-decoration: line-through; opacity: .7; }
.bt-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bt-x { display: grid; place-items: center; width: 16px; height: 16px; border-radius: 5px; color: var(--muted); }
.bt-x:hover { background: var(--line); color: var(--ink); }
.bt-agents { display: flex; align-items: center; gap: 5px; flex-wrap: wrap; min-width: 0; }
.bt-label { font-size: 11px; color: var(--muted); }
.bt-agent { display: inline-flex; align-items: center; gap: 5px; height: 24px; padding: 0 10px; border-radius: 999px; font-size: 11.5px; background: var(--soft); border: 1px solid var(--line) !important; }
.bt-agent:not(:disabled):hover { border-color: var(--accent) !important; color: var(--accent); }
.bt-agent:disabled { opacity: .45; }
.bt-btn { display: inline-flex; align-items: center; justify-content: center; gap: 4px; flex-shrink: 0; height: 26px; min-width: 26px; padding: 0 7px; border-radius: 8px; font-size: 11.5px; color: var(--muted); }
.bt-btn:not(:disabled):hover { background: var(--soft); color: var(--ink); }
.bt-btn:disabled { opacity: .45; cursor: default; }
.bt-body { display: flex; flex-direction: column; height: 300px; border: 1px solid var(--line); border-radius: 10px; overflow: hidden; background: var(--surface); }
.bt-empty { margin: auto; color: var(--muted); font-size: 12px; }
.bt-problem { margin: 0; font-size: 11.5px; color: var(--danger); }
.bt-problem button { color: inherit; text-decoration: underline; }
</style>

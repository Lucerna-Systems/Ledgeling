<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Bot, Columns2, Rows2, SquareTerminal } from '@lucide/vue'
import TerminalGrid from './TerminalGrid.vue'
import ConfirmDialog from './ConfirmDialog.vue'
import { focused, initPanes, openAgent, openShell } from './panes'
import { terms } from './terminals'
import { splitHint } from './shortcuts'
import { claude, info, problem, refresh, scope, statusOf } from './terminalState'

/** A pet's terminal window: a shell or Claude Code in the folder it works in,
 *  with the pet showing when an agent is working or waiting for you. */
const folder = computed(() => scope.value ?? '')
const context = computed(() => info.value.context)
const focusedId = computed(() => focused.get(folder.value) ?? null)

/** Opening the window should land in a terminal, not on a blank page. A
 *  folder with nothing running gets a shell; Claude Code is one click away. */
let starting = false
async function ensureTerminal() {
  if (!folder.value || starting) return
  starting = true
  try {
    await initPanes()
    if (![...terms.values()].some(t => t.alive && t.worktree === folder.value)) openShell('row', null)
  } finally { starting = false }
}
// Runs when the first read finds the folder, and again when another pet takes the window.
watch(folder, () => void ensureTerminal())

let timer: ReturnType<typeof setInterval> | undefined
let unlisten: UnlistenFn | undefined
onMounted(async () => {
  await refresh()
  unlisten = await listen('terminal:changed', () => void refresh())
  // Agent status and who is still alive change without any click.
  timer = setInterval(() => { if (!document.hidden) void refresh() }, 1500)
})
onUnmounted(() => { clearInterval(timer); unlisten?.() })
</script>

<template>
  <div class="term-shell">
    <header class="term-header">
      <SquareTerminal :size="15" class="term-glyph" />
      <div class="term-where">
        <strong>{{ context?.petName ?? 'Terminal' }}</strong>
        <small :title="folder">{{ folder }}</small>
      </div>
      <span class="grow" />
      <button v-for="agent in info.agents" :key="agent.id" class="ghost" :disabled="!folder" :title="`Start ${agent.name} beside the focused terminal`" @click="openAgent(agent)"><Bot :size="13" /> {{ agent.name }}</button>
      <button class="icon" :disabled="!folder" :title="`New terminal on the right (${splitHint('row')})`" @click="openShell('row', focusedId)"><Columns2 :size="14" /></button>
      <button class="icon" :disabled="!folder" :title="`New terminal below (${splitHint('column')})`" @click="openShell('column', focusedId)"><Rows2 :size="14" /></button>
    </header>
    <p v-if="problem" class="banner error" role="alert"><span>{{ problem }}</span><button class="ghost" @click="problem = ''">Dismiss</button></p>
    <TerminalGrid v-if="folder" :scope="folder" :active="true" :claude="claude" :agents="info.agents" :bar="false" :status="statusOf"
                  :heading="`Terminals for ${context?.petName ?? 'your pet'}`"
                  blurb="Run commands in the folder this pet works in. Start Claude Code and the pet shows when it is working, and when it needs your answer.">
      <template #hint>
        <p v-if="!info.agents.length" class="small">No coding agents were found on your PATH. Install Claude Code, Codex or Gemini CLI, then reopen this window.</p>
      </template>
    </TerminalGrid>
    <ConfirmDialog />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, provide, type Component } from 'vue'
import { Bot, Columns2, Rows2, SquareTerminal } from '@lucide/vue'
import PaneNode from './PaneNode.vue'
import { closePane, focused, initPanes, layouts, openAgent, openClaude, openShell, setLayout } from './panes'
import { terms } from './terminals'
import { splitHint, splitShortcut } from './shortcuts'
import { gridKey } from './gridContext'
import { ask } from './confirm'
import type { Direction } from './layout'
import type { AgentStatus, AgentTool } from './types'

/** A folder's terminals as split panes, shared by coding mode and terminal
 *  mode. Only an `active` grid holds the terminals and answers the split keys.
 *  The host says what is on screen and fills in the bits that are its own. */
const props = withDefaults(defineProps<{
  scope: string
  active: boolean
  /** Whether the user's Claude Code is installed. */
  claude: boolean
  /** Every agent installed, when the host knows; otherwise just Claude Code is offered. */
  agents?: AgentTool[]
  heading: string
  blurb: string
  /** Show the Claude / split buttons in a bar; the wide coding layout shows them elsewhere. */
  bar?: boolean
  status?: (id: number) => AgentStatus | null
  /** The host's own buttons on each pane. */
  tools?: { component: Component; props(id: number): Record<string, unknown> }
}>(), { bar: true, status: () => () => null, tools: undefined, agents: undefined })

const offered = computed<AgentTool[]>(() => props.agents ?? (props.claude ? [{ id: 'claude', name: 'Claude Code', command: 'claude' }] : []))

const layout = computed(() => layouts.get(props.scope) ?? null)
const focusedId = computed(() => focused.get(props.scope) ?? null)

const split = (near: number | null, dir: Direction) => openShell(dir, near)
async function close(id: number) {
  const entry = terms.get(id)
  if (entry?.alive && !await ask(`Stop ${entry.title}? Whatever it is running ends too.`, `Stop ${entry.title}`)) return
  await closePane(id)
}

provide(gridKey, {
  worktree: computed(() => props.scope),
  active: computed(() => props.active),
  focused: focusedId,
  focus: id => { if (props.scope) focused.set(props.scope, id) },
  split,
  close: id => void close(id),
  changed: () => setLayout(props.scope, layout.value),
  status: id => props.status(id),
  tools: props.tools ?? null,
})

function onKey(event: KeyboardEvent) {
  if (!props.active) return
  const dir = splitShortcut(event)
  if (!dir) return
  event.preventDefault()
  split(focusedId.value, dir)
}
onMounted(async () => {
  window.addEventListener('keydown', onKey)
  await initPanes()
})
onUnmounted(() => window.removeEventListener('keydown', onKey))
defineExpose({ split, startClaude: openClaude })
</script>

<template>
  <section class="grid-wrap">
    <div v-if="bar" class="grid-bar">
      <span class="grow" />
      <button v-for="agent in offered" :key="agent.id" class="ghost" :title="`Start ${agent.name} beside the focused terminal`" @click="openAgent(agent)"><Bot :size="13" /> {{ agent.name }}</button>
      <button class="icon" :title="`New terminal on the right (${splitHint('row')})`" @click="split(focusedId, 'row')"><Columns2 :size="14" /></button>
      <button class="icon" :title="`New terminal below (${splitHint('column')})`" @click="split(focusedId, 'column')"><Rows2 :size="14" /></button>
    </div>
    <div v-if="layout" class="grid-root"><PaneNode :node="layout" /></div>
    <div v-else class="empty">
      <span class="glyph"><SquareTerminal :size="22" /></span>
      <h2>{{ heading }}</h2>
      <p>{{ blurb }}</p>
      <div class="card-actions">
        <button v-for="(agent, i) in offered" :key="agent.id" :class="{ 'primary-button': i === 0 }" @click="openAgent(agent)"><Bot :size="14" /> Start {{ agent.name }}</button>
        <button @click="split(null, 'row')"><SquareTerminal :size="14" /> Open a shell</button>
      </div>
      <slot name="hint" />
    </div>
  </section>
</template>

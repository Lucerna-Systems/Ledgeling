<script setup lang="ts">
import { computed, onMounted, onUnmounted, provide, watch } from 'vue'
import { Bot, Columns2, Rows2, SquareTerminal } from '@lucide/vue'
import PaneNode from './PaneNode.vue'
import { closePane, focused, initPanes, layouts, openClaude, openShell, setLayout, spawnPane } from '../panes'
import { terms } from '../terminals'
import { pendingTerminal } from '../terminalFocus'
import { current, currentPet, root, tab, wide } from '../state'
import { useLive } from '../live'
import { has, loadTools } from '../tools'
import { splitHint, splitShortcut } from '../shortcuts'
import { gridKey } from '../gridContext'
import type { Direction } from '../layout'
import { ask } from '../confirm'

/** The worktree on screen as split terminals. Only an `active` grid holds
 *  the terminals and answers the split keys. */
const props = defineProps<{ active: boolean }>()

const worktree = computed(() => root.value ?? '')
const layout = computed(() => layouts.get(worktree.value) ?? null)
const focusedId = computed(() => focused.get(worktree.value) ?? null)
const claude = computed(() => has('claude'))

useLive(() => props.active && [...terms.values()].some(t => t.alive && t.worktree === worktree.value))

const split = (near: number | null, dir: Direction) => openShell(dir, near)
const startClaude = openClaude
async function close(id: number) {
  const entry = terms.get(id)
  if (entry?.alive && !await ask(`Stop ${entry.title}? Whatever it is running ends too.`, `Stop ${entry.title}`)) return
  await closePane(id)
}

provide(gridKey, {
  worktree,
  active: computed(() => props.active),
  focused: focusedId,
  focus: id => { if (worktree.value) focused.set(worktree.value, id) },
  split,
  close: id => void close(id),
  changed: () => setLayout(worktree.value, layout.value),
})

/** Another tab asked for a terminal: an AI setup prompt or an installer. */
async function takePending() {
  const pending = pendingTerminal.value
  if (!pending || !props.active) return
  pendingTerminal.value = null
  const entry = await spawnPane({ worktree: pending.worktree, kind: pending.kind ?? 'shell', command: pending.command ?? null, title: pending.title, tool: pending.tool }, focusedId.value)
  if (entry && pending.tool) setTimeout(() => void loadTools(true), 4000)
}
watch([() => props.active, pendingTerminal], () => void takePending())

function onKey(event: KeyboardEvent) {
  if (!props.active) return
  const dir = splitShortcut(event)
  if (!dir) return
  event.preventDefault()
  split(focusedId.value, dir)
}
onMounted(async () => {
  window.addEventListener('keydown', onKey)
  await loadTools()
  await initPanes()
  void takePending()
})
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <section class="grid-wrap">
    <!-- The wide layout shows these beside its Terminals and Editor tabs. -->
    <div v-if="!wide" class="grid-bar">
      <span class="grow" />
      <button class="ghost" :disabled="!claude" :title="claude ? 'Start Claude Code beside the focused terminal' : 'Claude Code is not installed. See Add-ons in the Worktrees tab.'" @click="startClaude"><Bot :size="13" /> Claude Code</button>
      <button class="icon" :title="`New terminal on the right (${splitHint('row')})`" @click="split(focusedId, 'row')"><Columns2 :size="14" /></button>
      <button class="icon" :title="`New terminal below (${splitHint('column')})`" @click="split(focusedId, 'column')"><Rows2 :size="14" /></button>
    </div>
    <div v-if="layout" class="grid-root"><PaneNode :node="layout" /></div>
    <div v-else class="empty">
      <span class="glyph"><SquareTerminal :size="22" /></span>
      <h2>Terminals for {{ current?.worktree.name }}</h2>
      <p>Commands run inside this worktree. Split terminals side by side or stacked, and open whatever they serve from the button on each one. Start Claude Code and {{ currentPet?.name ?? 'the pet' }} shows when it is working or waiting for you.</p>
      <div class="card-actions">
        <button class="primary-button" :disabled="!claude" @click="startClaude"><Bot :size="14" /> Start Claude Code</button>
        <button @click="split(null, 'row')"><SquareTerminal :size="14" /> Open a shell</button>
      </div>
      <p v-if="!claude" class="small">Claude Code is not installed yet. <button class="text-link bare" @click="tab = 'worktrees'">Install it from Add-ons</button></p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import SharedGrid from '../../terminal/TerminalGrid.vue'
import OpenMenu from './OpenMenu.vue'
import { focused, spawnPane } from '../../terminal/panes'
import { terms } from '../../terminal/terminals'
import { pendingTerminal } from '../terminalFocus'
import { codeState, current, currentPet, root, tab, wide } from '../state'
import { useLive } from '../live'
import { has, loadTools } from '../tools'

/** The worktree on screen as split terminals, with coding mode's extras:
 *  ports and apps on each pane, installers, and AI setup prompts from other tabs. */
const props = defineProps<{ active: boolean }>()

const worktree = computed(() => root.value ?? '')
const focusedId = computed(() => focused.get(worktree.value) ?? null)
const claude = computed(() => has('claude'))

useLive(() => props.active && [...terms.values()].some(t => t.alive && t.worktree === worktree.value))

const tools = { component: OpenMenu, props: (id: number) => ({ session: id, worktree: worktree.value }) }
const status = (id: number) => codeState.value.sessions.find(s => s.id === id)?.agent ?? null

/** Another tab asked for a terminal: an AI setup prompt or an installer. */
async function takePending() {
  const pending = pendingTerminal.value
  if (!pending || !props.active) return
  pendingTerminal.value = null
  const entry = await spawnPane({ worktree: pending.worktree, kind: pending.kind ?? 'shell', command: pending.command ?? null, title: pending.title, tool: pending.tool }, focusedId.value)
  if (entry && pending.tool) setTimeout(() => void loadTools(true), 4000)
}
watch([() => props.active, pendingTerminal], () => void takePending())
onMounted(async () => {
  await loadTools()
  void takePending()
})
</script>

<template>
  <SharedGrid :scope="worktree" :active="active" :claude="claude" :bar="!wide" :status="status" :tools="tools"
              :heading="`Terminals for ${current?.worktree.name ?? ''}`"
              :blurb="`Commands run inside this worktree. Split terminals side by side or stacked, and open whatever they serve from the button on each one. Start Claude Code and ${currentPet?.name ?? 'the pet'} shows when it is working or waiting for you.`">
    <template #hint>
      <p v-if="!claude" class="small">Claude Code is not installed yet. <button class="text-link bare" @click="tab = 'worktrees'">Install it from Add-ons</button></p>
    </template>
  </SharedGrid>
</template>

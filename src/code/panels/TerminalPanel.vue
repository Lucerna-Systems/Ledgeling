<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Bot, Plus, SquareTerminal, X } from '@lucide/vue'
import TermView from '../components/TermView.vue'
import { adopt, close, spawn, terms } from '../terminals'
import { pendingTerminal } from '../terminalFocus'
import { current, currentPet, refreshState, root, tab } from '../state'
import { has, loadTools } from '../tools'

const props = defineProps<{ active?: boolean }>()
const selectedId = ref<number | null>(null)

/** Only this worktree's terminals, plus installers, which belong to no worktree. */
const list = computed(() => [...terms.values()].filter(t => t.kind !== 'run' && (t.worktree === root.value || t.worktree === '')))
const selected = computed(() => list.value.find(t => t.id === selectedId.value) ?? list.value.at(-1) ?? null)
const claude = computed(() => has('claude'))

async function open(kind: 'shell' | 'agent') {
  if (!root.value) return
  const entry = await spawn({ worktree: root.value, kind, command: kind === 'agent' ? 'claude' : null, title: kind === 'agent' ? 'Claude Code' : 'Shell' })
  if (entry) selectedId.value = entry.id
  void refreshState()
}
async function closeTerm(id: number) {
  const entry = terms.get(id)
  if (entry?.alive && !confirm(`Stop ${entry.title}? Whatever it is running ends too.`)) return
  await close(id)
  void refreshState()
}

async function takePending() {
  const pending = pendingTerminal.value
  if (!pending) return
  pendingTerminal.value = null
  const entry = await spawn({ worktree: pending.worktree, kind: pending.kind ?? 'shell', command: pending.command ?? null, title: pending.title, tool: pending.tool })
  if (entry) selectedId.value = entry.id
  if (pending.tool) setTimeout(() => void loadTools(true), 4000)
  void refreshState()
}
watch([() => props.active, pendingTerminal], ([active]) => { if (active) void takePending() })
onMounted(async () => { await loadTools(); await adopt(); void takePending() })
</script>

<template>
  <section>
    <div class="term-tabs" role="tablist" aria-label="Terminals">
      <div v-for="t in list" :key="t.id" class="term-tab" :class="{ active: t.id === selected?.id }" role="tab" :aria-selected="t.id === selected?.id" @click="selectedId = t.id">
        <component :is="t.kind === 'agent' ? Bot : SquareTerminal" :size="12" />
        <span>{{ t.title }}</span><span v-if="!t.alive" class="muted small">· ended</span>
        <button class="icon" :title="`Close ${t.title}`" @click.stop="closeTerm(t.id)"><X :size="11" /></button>
      </div>
      <span style="flex:1" />
      <button class="ghost" :disabled="!claude" :title="claude ? 'Start Claude Code in this worktree' : 'Claude Code is not installed. See Add-ons in the Worktrees tab.'" @click="open('agent')"><Bot :size="13" /> Claude Code</button>
      <button class="icon" title="New shell" @click="open('shell')"><Plus :size="14" /></button>
    </div>
    <TermView v-if="selected" :id="selected.id" :key="selected.id" :active="props.active" />
    <div v-else class="empty">
      <span class="glyph"><SquareTerminal :size="22" /></span>
      <h2>Terminal for {{ current?.worktree.name }}</h2>
      <p>Commands run inside this worktree. Start Claude Code and {{ currentPet?.name ?? 'the pet' }} shows when it is working or waiting for you.</p>
      <div class="card-actions">
        <button class="primary-button" :disabled="!claude" @click="open('agent')"><Bot :size="14" /> Start Claude Code</button>
        <button @click="open('shell')"><SquareTerminal :size="14" /> Open a shell</button>
      </div>
      <p v-if="!claude" class="small">Claude Code is not installed yet. <button class="text-link bare" @click="tab = 'worktrees'">Install it from Add-ons</button></p>
    </div>
  </section>
</template>

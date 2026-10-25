<script setup lang="ts">
import { computed, inject } from 'vue'
import { Bot, Columns2, Play, Rows2, SquareTerminal, X } from '@lucide/vue'
import TermView from './TermView.vue'
import AgentChip from './AgentChip.vue'
import RenameTitle from './RenameTitle.vue'
import { rename, terms } from './terminals'
import { splitHint } from './shortcuts'
import { gridKey } from './gridContext'

const props = defineProps<{ id: number }>()
const grid = inject(gridKey)!
const entry = computed(() => terms.get(props.id))
const icon = computed(() => entry.value?.kind === 'agent' ? Bot : entry.value?.kind === 'run' ? Play : SquareTerminal)
const focused = computed(() => grid.focused.value === props.id)
const status = computed(() => entry.value?.alive ? grid.status(props.id) : null)
</script>

<template>
  <div v-if="entry" class="pane" :class="{ focused, ended: !entry.alive }" @pointerdown="grid.focus(id)" @focusin="grid.focus(id)">
    <div class="pane-head">
      <component :is="icon" :size="12" class="pane-icon" />
      <span class="pane-title"><RenameTitle :title="entry.title" @rename="rename(id, $event)" /></span>
      <AgentChip v-if="status" :status="status" />
      <span v-if="!entry.alive" class="muted small">ended{{ entry.exitCode ? ` · code ${entry.exitCode}` : '' }}</span>
      <span class="grow" />
      <component :is="grid.tools.component" v-if="grid.tools" v-bind="grid.tools.props(id)" />
      <button class="icon" :title="`Split right (${splitHint('row')})`" @click.stop="grid.split(id, 'row')"><Columns2 :size="13" /></button>
      <button class="icon" :title="`Split down (${splitHint('column')})`" @click.stop="grid.split(id, 'column')"><Rows2 :size="13" /></button>
      <button class="icon" :title="`Close ${entry.title}`" @click.stop="grid.close(id)"><X :size="13" /></button>
    </div>
    <TermView :id="id" :active="grid.active.value" :focus="focused" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { FolderGit2, FolderPlus, GitBranchPlus } from '@lucide/vue'
import PetFace from '../../assistant/PetFace.vue'
import { activityOf, codeState, current, petFor, select, tab } from '../state'
import type { WorktreeView } from '../types'

const emit = defineEmits<{ close: []; addProject: [] }>()

async function choose(tree: WorktreeView) {
  await select(tree.path).catch(() => {})
  emit('close')
}
function newWorktree() {
  tab.value = 'worktrees'
  emit('close')
}
const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') emit('close') }
onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div class="popover-scrim" @click="emit('close')" />
  <div class="popover" role="dialog" aria-label="Worktrees">
    <template v-for="project in codeState.projects" :key="project.path">
      <div class="group"><span>{{ project.name }}</span></div>
      <p v-if="project.problem" class="banner warn"><span>{{ project.problem }}</span></p>
      <button v-for="tree in project.worktrees" :key="tree.path" class="tree-row" :class="{ selected: tree.path === current?.worktree.path }" @click="choose(tree)">
        <PetFace :pet="petFor(tree) ? { packId: petFor(tree)!.packId, name: petFor(tree)!.name } : null" :pet-id="tree.petId" :size="26" />
        <span class="grow">
          <strong>{{ tree.name }}</strong>
          <small>{{ tree.branch ?? 'detached' }} · {{ petFor(tree)?.name ?? 'no pet' }}{{ tree.main ? ' · main checkout' : '' }}</small>
        </span>
        <span v-if="activityOf(tree.path)" class="chip" :class="activityOf(tree.path)!.tone">{{ activityOf(tree.path)!.label }}</span>
      </button>
    </template>
    <div class="popover-actions">
      <button @click="newWorktree"><GitBranchPlus :size="14" /> New worktree</button>
      <button @click="emit('addProject'); emit('close')"><FolderPlus :size="14" /> Add project</button>
      <span class="grow" />
      <span class="muted small" style="align-self:center"><FolderGit2 :size="11" /> {{ codeState.projects.length }} project{{ codeState.projects.length === 1 ? '' : 's' }}</span>
    </div>
  </div>
</template>

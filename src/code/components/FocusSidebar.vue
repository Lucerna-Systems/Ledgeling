<script setup lang="ts">
import { computed, reactive } from 'vue'
import { ChevronRight, FolderGit2, FolderPlus, FolderTree, GitBranch, GitBranchPlus, GitCompareArrows, Globe, PanelLeftClose, SquareTerminal } from '@lucide/vue'
import PetFace from '../../assistant/PetFace.vue'
import { activityOf, changeCount, code, codeState, current, petFor, refreshState, select, tab } from '../state'
import type { Tab } from '../types'
import { live, urlFor, useLive } from '../live'
import type { WorktreeView } from '../types'

/** The wide layout's worktree list: every project and worktree, who works
 *  there, what is running and what it serves, one click from switching. */
const props = defineProps<{ active?: boolean }>()
const emit = defineEmits<{ collapse: [] }>()

useLive(() => !!props.active && codeState.value.sessions.some(s => s.alive))

const sessionsOf = (tree: WorktreeView) => codeState.value.sessions.filter(s => s.worktree === tree.path && s.alive)
const portsOf = (tree: WorktreeView) => {
  const ids = new Set(sessionsOf(tree).map(s => s.id))
  return [...new Set(live.value.ports.filter(p => p.session !== null && ids.has(p.session)).map(p => p.port))]
}
const appsOf = (tree: WorktreeView) => {
  const ids = new Set(sessionsOf(tree).map(s => s.id))
  return live.value.apps.filter(a => ids.has(a.session))
}
const selected = computed(() => current.value?.worktree.path)

/** Worktrees opened or closed by hand; the one on screen is open unless closed. */
const toggled = reactive(new Map<string, boolean>())
const expanded = (tree: WorktreeView) => toggled.get(tree.path) ?? tree.path === selected.value
function toggle(tree: WorktreeView) { toggled.set(tree.path, !expanded(tree)) }

async function choose(tree: WorktreeView) {
  if (tree.path !== selected.value) await select(tree.path).catch(() => {})
}
/** Files and Changes belong to one worktree: switch to it, then show the view. */
async function show(tree: WorktreeView, view: Tab) {
  await choose(tree)
  tab.value = view
}
async function addProject() {
  try { await code('code_add_project'); await refreshState() } catch { /* banner */ }
}
function newWorktree() {
  tab.value = 'worktrees'
}
const browse = (port: number) => void code('code_open_url', { url: urlFor(port) }).catch(() => {})
const activate = (pid: number) => void code('code_activate', { pid }).catch(() => {})
</script>

<template>
  <aside class="focus-side" aria-label="Worktrees">
    <div class="side-head">
      <span class="grow">Worktrees</span>
      <button class="icon" title="Add project" @click="addProject"><FolderPlus :size="14" /></button>
      <button class="icon" title="Hide the side panel" @click="emit('collapse')"><PanelLeftClose :size="14" /></button>
    </div>
    <div class="side-scroll">
      <template v-for="project in codeState.projects" :key="project.path">
        <div class="side-group">
          <FolderGit2 :size="12" /><span class="grow" :title="project.path">{{ project.name }}</span>
          <button class="icon" :title="`New worktree in ${project.name}`" @click="newWorktree"><GitBranchPlus :size="13" /></button>
        </div>
        <p v-if="project.problem" class="banner warn"><span>{{ project.problem }}</span></p>
        <template v-for="tree in project.worktrees" :key="tree.path">
        <div
          class="side-row" :class="{ selected: tree.path === selected }"
          role="button" tabindex="0" :aria-current="tree.path === selected" @click="choose(tree)" @keydown.enter="choose(tree)"
        >
          <button class="bare side-caret" :class="{ open: expanded(tree) }" :aria-expanded="expanded(tree)" :title="expanded(tree) ? 'Hide files and changes' : 'Show files and changes'" @click.stop="toggle(tree)"><ChevronRight :size="12" /></button>
          <PetFace :pet="petFor(tree) ? { packId: petFor(tree)!.packId, name: petFor(tree)!.name } : null" :pet-id="tree.petId" :size="24" />
          <span class="side-text">
            <strong>{{ tree.name }}</strong>
            <small><GitBranch :size="10" /> {{ tree.branch ?? 'detached' }}</small>
          </span>
          <span class="side-meta">
            <span v-if="activityOf(tree.path)" class="dot" :class="activityOf(tree.path)!.tone === 'idle' ? 'idle' : activityOf(tree.path)!.tone === 'live' ? 'live' : 'busy'" :title="activityOf(tree.path)!.label" />
            <span v-if="sessionsOf(tree).length" class="side-count" :title="`${sessionsOf(tree).length} running`"><SquareTerminal :size="10" />{{ sessionsOf(tree).length }}</span>
          </span>
          <span v-if="portsOf(tree).length || appsOf(tree).length" class="side-links">
            <button v-for="app in appsOf(tree)" :key="app.pid" class="port-pill" :title="`Bring ${app.name} forward`" @click.stop="activate(app.pid)">{{ app.name }}</button>
            <button v-for="port in portsOf(tree)" :key="port" class="port-pill" :title="`Open ${urlFor(port)}`" @click.stop="browse(port)"><Globe :size="10" />:{{ port }}</button>
          </span>
        </div>
        <div v-if="expanded(tree)" class="side-subs" role="group" :aria-label="`${tree.name} views`">
          <button class="side-sub" @click="show(tree, 'files')"><FolderTree :size="13" /> Files</button>
          <button class="side-sub" :class="{ on: tree.path === selected && tab === 'changes' }" @click="show(tree, 'changes')">
            <GitCompareArrows :size="13" /> Changes
            <span v-if="tree.path === selected && changeCount" class="side-badge">{{ changeCount > 99 ? '99+' : changeCount }}</span>
          </button>
        </div>
        </template>
      </template>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ChevronRight, File, FilePlus, Folder, FolderOpen, FolderPlus, Pencil, RefreshCw, Trash2, ExternalLink } from '@lucide/vue'
import { code, fileChanges, fileTick, openFile, openRequest, root, status } from '../state'
import type { Entry } from '../types'

const listings = reactive(new Map<string, Entry[]>())
const expanded = reactive(new Set<string>())
const flashing = reactive(new Set<string>())
const creating = ref<{ dir: string; folder: boolean } | null>(null)
const renaming = ref<string | null>(null)
const draft = ref('')

async function load(dir: string) {
  if (!root.value) return
  try { listings.set(dir, await code<Entry[]>('code_list', { worktree: root.value, dir })) } catch { /* banner */ }
}
function reset() {
  listings.clear(); expanded.clear()
  void load('')
}
watch(root, reset, { immediate: true })

/** Folders whose listing changed, reloaded if they are showing; changed files flash. */
watch(fileTick, () => {
  const dirs = new Set<string>()
  for (const path of fileChanges.value.paths) {
    const parent = path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : ''
    dirs.add(parent)
    flashing.add(path)
    setTimeout(() => flashing.delete(path), 1200)
  }
  for (const dir of dirs) if (dir === '' || expanded.has(dir)) void load(dir)
})

const letters = computed(() => {
  const map = new Map<string, string>()
  for (const file of status.value?.files ?? []) {
    const letter = file.conflicted ? 'C' : file.untracked ? 'U' : (file.unstaged !== '.' ? file.unstaged : file.staged)
    map.set(file.path, letter)
    // Folders carry a dot when something inside them changed.
    let at = file.path.lastIndexOf('/')
    while (at > 0) { const dir = file.path.slice(0, at); if (!map.has(dir)) map.set(dir, '•'); at = dir.lastIndexOf('/') }
  }
  return map
})

interface Row { entry: Entry; depth: number }
const rows = computed<Row[]>(() => {
  const out: Row[] = []
  const walk = (dir: string, depth: number) => {
    for (const entry of listings.get(dir) ?? []) {
      out.push({ entry, depth })
      if (entry.dir && expanded.has(entry.path)) walk(entry.path, depth + 1)
    }
  }
  walk('', 0)
  return out
})

function toggle(entry: Entry) {
  if (!entry.dir) { openFile(entry.path); return }
  if (expanded.has(entry.path)) expanded.delete(entry.path)
  else { expanded.add(entry.path); if (!listings.has(entry.path)) void load(entry.path) }
}

const selectedPath = computed(() => openRequest.value?.path ?? null)

function startCreate(folder: boolean) {
  const dir = selectedPath.value && !listings.get(selectedPath.value) ? selectedPath.value.split('/').slice(0, -1).join('/') : ''
  creating.value = { dir, folder }
  draft.value = ''
}
async function finishCreate() {
  const pending = creating.value
  creating.value = null
  const name = draft.value.trim()
  if (!pending || !name || !root.value) return
  const path = pending.dir ? `${pending.dir}/${name}` : name
  try {
    await code('code_file_op', { worktree: root.value, op: pending.folder ? 'create-folder' : 'create-file', path, to: '' })
    if (pending.dir) expanded.add(pending.dir)
    await load(pending.dir)
    if (!pending.folder) openFile(path)
  } catch { /* banner */ }
}
function startRename(entry: Entry) { renaming.value = entry.path; draft.value = entry.name }
async function finishRename(entry: Entry) {
  const name = draft.value.trim()
  renaming.value = null
  if (!name || name === entry.name || !root.value) return
  const parent = entry.path.includes('/') ? entry.path.slice(0, entry.path.lastIndexOf('/')) : ''
  try {
    await code('code_file_op', { worktree: root.value, op: 'rename', path: entry.path, to: parent ? `${parent}/${name}` : name })
    await load(parent)
  } catch { /* banner */ }
}
async function remove(entry: Entry) {
  if (!root.value || !confirm(`Delete ${entry.path}${entry.dir ? ' and everything in it' : ''}? This cannot be undone here.`)) return
  const parent = entry.path.includes('/') ? entry.path.slice(0, entry.path.lastIndexOf('/')) : ''
  try { await code('code_file_op', { worktree: root.value, op: 'delete', path: entry.path, to: '' }); await load(parent) } catch { /* banner */ }
}
async function reveal() {
  if (root.value) await code('code_file_op', { worktree: root.value, op: 'reveal', path: selectedPath.value ?? '', to: '' }).catch(() => {})
}
</script>

<template>
  <section>
    <div class="panel-bar">
      <h2 class="grow">Files</h2>
      <button class="icon" title="New file" @click="startCreate(false)"><FilePlus :size="15" /></button>
      <button class="icon" title="New folder" @click="startCreate(true)"><FolderPlus :size="15" /></button>
      <button class="icon" title="Show in Finder / Explorer" @click="reveal"><ExternalLink :size="15" /></button>
      <button class="icon" title="Refresh" @click="reset"><RefreshCw :size="14" /></button>
    </div>
    <div class="panel-scroll">
      <ul class="rows" role="tree" aria-label="Project files">
        <li v-if="creating && creating.dir === ''" class="row">
          <component :is="creating.folder ? Folder : File" :size="14" class="file-icon" />
          <input v-model="draft" v-focus class="grow" :placeholder="creating.folder ? 'Folder name' : 'File name, e.g. src/app.ts'" @keydown.enter="finishCreate" @keydown.escape="creating = null" @blur="finishCreate">
        </li>
        <template v-for="{ entry, depth } in rows" :key="entry.path">
          <li class="row" role="treeitem" :aria-expanded="entry.dir ? expanded.has(entry.path) : undefined"
              :class="{ dir: entry.dir, ignored: entry.ignored, selected: selectedPath === entry.path, flash: flashing.has(entry.path) }"
              :style="{ paddingLeft: `${6 + depth * 14}px` }" @click="toggle(entry)">
            <ChevronRight v-if="entry.dir" :size="12" class="chev" :class="{ open: expanded.has(entry.path) }" />
            <span v-else style="width:12px" />
            <component :is="entry.dir ? (expanded.has(entry.path) ? FolderOpen : Folder) : File" :size="14" class="file-icon" />
            <input v-if="renaming === entry.path" v-model="draft" v-focus class="grow" @click.stop @keydown.enter="finishRename(entry)" @keydown.escape="renaming = null" @blur="finishRename(entry)">
            <span v-else class="name">{{ entry.name }}</span>
            <span class="hover-actions">
              <button class="icon" title="Rename" @click.stop="startRename(entry)"><Pencil :size="12" /></button>
              <button class="icon" title="Delete" @click.stop="remove(entry)"><Trash2 :size="12" /></button>
            </span>
            <span v-if="letters.get(entry.path)" class="letter" :class="letters.get(entry.path)">{{ letters.get(entry.path) }}</span>
          </li>
          <li v-if="creating && entry.dir && creating.dir === entry.path && expanded.has(entry.path)" class="row" :style="{ paddingLeft: `${20 + (depth + 1) * 14}px` }">
            <component :is="creating.folder ? Folder : File" :size="14" class="file-icon" />
            <input v-model="draft" v-focus class="grow" @keydown.enter="finishCreate" @keydown.escape="creating = null" @blur="finishCreate">
          </li>
        </template>
      </ul>
    </div>
  </section>
</template>

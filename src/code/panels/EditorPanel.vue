<script setup lang="ts">
import { computed, markRaw, onBeforeUnmount, onMounted, ref, shallowReactive, watch } from 'vue'
import { ChevronRight, FileCode2, GitCompareArrows, Save, X } from '@lucide/vue'
import { languageFor, monaco, setupMonaco } from '../monaco'
import { code, fileChanges, fileTick, openRequest, root, short, showDiff } from '../state'
import type { FileText } from '../types'

const props = defineProps<{ active?: boolean }>()

interface Open {
  path: string
  model: monaco.editor.ITextModel
  /** Disk mtime this buffer was read or saved at. */
  modified: number
  saved: number
  version: number
  view: monaco.editor.ICodeEditorViewState | null
  conflict: boolean
  notice: string
}

/** Open files per worktree, so switching worktrees and back keeps your tabs. */
const byWorktree = new Map<string, Open[]>()
const files = shallowReactive<Open[]>([])
const activePath = ref<string | null>(null)
const tick = ref(0)
const active = computed(() => { void tick.value; return files.find(f => f.path === activePath.value) ?? null })
const dirty = (file: Open) => { void tick.value; return file.model.getAlternativeVersionId() !== file.version }

const host = ref<HTMLDivElement | null>(null)
let editor: monaco.editor.IStandaloneCodeEditor | null = null

function uri(path: string) {
  return monaco.Uri.from({ scheme: 'file', path: `/${root.value ?? ''}/${path}`.replace(/\/+/g, '/') })
}

async function open(path: string, line?: number) {
  if (!root.value) return
  let file = files.find(f => f.path === path)
  if (!file) {
    let text: FileText
    try { text = await code<FileText>('code_read', { worktree: root.value, path }) } catch { return }
    const notice = text.binary ? 'This is a binary file, so it is not shown.' : text.tooLarge ? 'This file is over 5 MB, so it is not opened here.' : ''
    const existing = monaco.editor.getModel(uri(path))
    const model = markRaw(existing ?? monaco.editor.createModel(text.text, languageFor(path), uri(path)))
    if (existing) existing.setValue(text.text)
    file = { path, model, modified: text.modified, saved: text.modified, version: model.getAlternativeVersionId(), view: null, conflict: false, notice }
    files.push(file)
  }
  show(file)
  if (line && editor) { editor.revealLineInCenter(line); editor.setPosition({ lineNumber: line, column: 1 }) }
}

function show(file: Open) {
  const previous = active.value
  if (previous && editor) previous.view = editor.saveViewState()
  activePath.value = file.path
  if (!editor) return
  editor.setModel(file.notice ? null : file.model)
  if (file.view) editor.restoreViewState(file.view)
  editor.updateOptions({ readOnly: !!file.notice })
  editor.focus()
}

function closeFile(file: Open) {
  if (dirty(file) && !confirm(`${short(file.path)} has unsaved changes. Close it anyway?`)) return
  const index = files.indexOf(file)
  files.splice(index, 1)
  file.model.dispose()
  if (activePath.value === file.path) {
    const next = files[Math.min(index, files.length - 1)]
    if (next) show(next); else { activePath.value = null; editor?.setModel(null) }
  }
}

async function save(force = false) {
  const file = active.value
  if (!file || !root.value || file.notice) return
  try {
    const modified = await code<number>('code_write', { worktree: root.value, path: file.path, text: file.model.getValue(), expected: force ? 0 : file.modified })
    file.modified = modified
    file.version = file.model.getAlternativeVersionId()
    file.conflict = false
    tick.value++
  } catch (e) {
    if (String(e).includes('changed-on-disk')) { file.conflict = true; tick.value++ }
  }
}

async function reload(file: Open) {
  if (!root.value) return
  try {
    const text = await code<FileText>('code_read', { worktree: root.value, path: file.path })
    if (text.text !== file.model.getValue()) {
      // Keeps undo history and the cursor, so an agent's edit reads as an edit.
      file.model.pushEditOperations([], [{ range: file.model.getFullModelRange(), text: text.text }], () => null)
    }
    file.modified = text.modified
    file.version = file.model.getAlternativeVersionId()
    file.conflict = false
    tick.value++
  } catch { /* deleted: leave the buffer */ }
}

/** Files changed on disk follow along unless you have unsaved edits in them. */
watch(fileTick, () => {
  for (const path of fileChanges.value.paths) {
    const file = files.find(f => f.path === path)
    if (!file) continue
    if (dirty(file)) { file.conflict = true; tick.value++ } else void reload(file)
  }
})

watch(openRequest, request => { if (request) void open(request.path, request.line) })

watch(root, (next, previous) => {
  if (previous) byWorktree.set(previous, [...files])
  if (active.value && editor) active.value.view = editor.saveViewState()
  files.splice(0, files.length, ...(next ? byWorktree.get(next) ?? [] : []))
  activePath.value = null
  editor?.setModel(null)
  if (files[0]) show(files[0])
})

watch(() => props.active, now => { if (now) requestAnimationFrame(() => editor?.layout()) })

const crumbs = computed(() => active.value?.path.split('/') ?? [])

onMounted(() => {
  setupMonaco()
  if (!host.value) return
  editor = markRaw(monaco.editor.create(host.value, {
    automaticLayout: true,
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
    fontSize: 12.5,
    lineHeight: 19,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    renderLineHighlight: 'line',
    smoothScrolling: true,
    padding: { top: 8 },
    tabSize: 2,
    theme: 'ledgeling',
    model: null,
    stickyScroll: { enabled: true },
    bracketPairColorization: { enabled: true },
  }))
  editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => void save())
  editor.onDidChangeModelContent(() => { tick.value++ })
  if (openRequest.value) void open(openRequest.value.path, openRequest.value.line)
})
onBeforeUnmount(() => editor?.dispose())
</script>

<template>
  <section>
    <div v-if="files.length" class="editor-tabs" role="tablist" aria-label="Open files">
      <div v-for="file in files" :key="file.path" class="editor-tab" :class="{ active: file.path === activePath }" role="tab" :aria-selected="file.path === activePath" :title="file.path" @click="show(file)" @auxclick.middle="closeFile(file)">
        <span>{{ short(file.path) }}</span>
        <span v-if="dirty(file)" class="dirty" aria-label="Unsaved" />
        <button class="icon" :title="`Close ${short(file.path)}`" @click.stop="closeFile(file)"><X :size="12" /></button>
      </div>
    </div>
    <div v-if="active" class="crumbs">
      <template v-for="(part, index) in crumbs" :key="index"><ChevronRight v-if="index" :size="10" /><span>{{ part }}</span></template>
      <span class="grow" style="flex:1" />
      <button class="icon" title="See what changed in this file" @click="showDiff(active.path)"><GitCompareArrows :size="13" /></button>
      <button class="icon" :disabled="!dirty(active)" title="Save (⌘S)" @click="save()"><Save :size="13" /></button>
    </div>
    <div v-if="active?.conflict" class="banner warn" role="alert">
      <p>{{ short(active.path) }} changed on disk while you were editing it.</p>
      <div class="actions"><button @click="reload(active)">Use disk version</button><button @click="save(true)">Keep mine</button></div>
    </div>
    <p v-if="active?.notice" class="banner">{{ active.notice }}</p>
    <div v-show="active && !active.notice" ref="host" class="monaco-host" />
    <div v-if="!active" class="empty">
      <span class="glyph"><FileCode2 :size="22" /></span>
      <h2>No file open</h2>
      <p>Pick a file in the Files tab, or click a change to open it here. Edits made by an agent appear as they are written.</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, markRaw, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { Check, CircleDot, FileCode2, GitCommitHorizontal, Minus, Plus, RefreshCw, Undo2 } from '@lucide/vue'
import { languageFor, monaco, setupMonaco } from '../monaco'
import { code, diffRequest, fileChanges, fileTick, openFile, refreshStatus, root, short, status } from '../state'
import type { FileChange, FileText } from '../types'

const props = defineProps<{ active?: boolean }>()
const selected = ref<string | null>(null)
/** Follow jumps to whichever file was just written, to watch an agent work. */
const follow = ref(true)
const sideBySide = ref(window.innerWidth > 760)
const message = ref('')
const committing = ref(false)
const host = ref<HTMLDivElement | null>(null)
let diff: monaco.editor.IStandaloneDiffEditor | null = null
let models: monaco.editor.ITextModel[] = []
let diffHost: HTMLElement | null = null

const files = computed(() => status.value?.files ?? [])
const staged = computed(() => files.value.filter(f => !f.untracked && f.staged !== '.'))
const unstaged = computed(() => files.value.filter(f => f.untracked || f.unstaged !== '.' || f.conflicted))
const current = computed(() => files.value.find(f => f.path === selected.value) ?? null)
const letter = (f: FileChange, stagedSide: boolean) => f.conflicted ? 'C' : f.untracked ? 'U' : stagedSide ? f.staged : f.unstaged

async function render() {
  const file = current.value
  const path = root.value
  if (!file || !path || !host.value) return
  setupMonaco()
  // The host leaves the DOM whenever the list empties; a new one needs a new editor.
  if (diff && diffHost !== host.value) { diff.dispose(); diff = null }
  if (!diff) {
    diffHost = host.value
    diff = markRaw(monaco.editor.createDiffEditor(host.value, {
      automaticLayout: true,
      readOnly: true,
      originalEditable: false,
      renderSideBySide: sideBySide.value,
      useInlineViewWhenSpaceIsLimited: true,
      renderOverviewRuler: false,
      minimap: { enabled: false },
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
      fontSize: 12,
      lineHeight: 18,
      scrollBeyondLastLine: false,
      hideUnchangedRegions: { enabled: true, contextLineCount: 3, minimumLineCount: 4 },
      theme: 'ledgeling',
    }))
  }
  const [before, after] = await Promise.all([
    code<string>('code_original', { worktree: path, path: file.original ?? file.path, index: false }).catch(() => ''),
    code<FileText>('code_read', { worktree: path, path: file.path }).then(t => t.binary || t.tooLarge ? '(binary or very large file)' : t.text).catch(() => ''),
  ])
  if (current.value?.path !== file.path) return
  const language = languageFor(file.path)
  const next = [monaco.editor.createModel(before, language), monaco.editor.createModel(after, language)]
  const scroll = diff.getModifiedEditor().getScrollTop()
  const same = diff.getModel()?.modified.uri && models[1] && selected.value === file.path
  diff.setModel({ original: next[0], modified: next[1] })
  for (const model of models) model.dispose()
  models = next
  if (same) diff.getModifiedEditor().setScrollTop(scroll)
}

watch(selected, () => void nextTick(render))
watch(sideBySide, value => diff?.updateOptions({ renderSideBySide: value }))
watch(diffRequest, path => { if (path) { selected.value = path; follow.value = false; diffRequest.value = null } }, { immediate: true })
watch(files, list => {
  if (!list.length) { selected.value = null; return }
  if (!selected.value || !list.some(f => f.path === selected.value)) selected.value = list[0].path
}, { immediate: true })
watch(fileTick, () => {
  const touched = fileChanges.value.paths
  if (follow.value && touched.length) {
    // Status refreshes a moment after the watcher fires; pick the file then.
    setTimeout(() => {
      const hit = [...touched].reverse().find(p => files.value.some(f => f.path === p))
      if (hit && hit !== selected.value) selected.value = hit
      else if (hit) void render()
    }, 300)
  } else if (selected.value && touched.includes(selected.value)) void render()
})
watch(() => props.active, now => { if (now) { refreshStatus(); void nextTick(render) } })
watch(root, () => { selected.value = null })

async function git(action: string, paths: string[], untracked: string[] = []) {
  if (!root.value) return
  try { await code('code_git', { worktree: root.value, action, paths, untracked, message: message.value }) } catch { /* banner */ }
  refreshStatus()
}
function discard(file: FileChange) {
  const what = file.untracked ? `Delete the new file ${file.path}?` : `Throw away your changes to ${file.path}?`
  if (!confirm(`${what} This cannot be undone.`)) return
  void git('discard', file.untracked ? [] : [file.path], file.untracked ? [file.path] : [])
}
async function commit() {
  if (!root.value || !message.value.trim()) return
  committing.value = true
  try {
    // Staged files commit on their own; with nothing staged, everything goes in.
    await code('code_git', { worktree: root.value, action: 'commit', paths: staged.value.map(f => f.path), untracked: [], message: message.value })
    message.value = ''
  } catch { /* banner */ } finally { committing.value = false; refreshStatus() }
}
const totals = computed(() => files.value.reduce((sum, f) => ({ added: sum.added + (f.added ?? 0), removed: sum.removed + (f.removed ?? 0) }), { added: 0, removed: 0 }))

onBeforeUnmount(() => { diff?.dispose(); for (const model of models) model.dispose() })
</script>

<template>
  <section>
    <div class="panel-bar">
      <h2 class="grow">{{ files.length ? `${files.length} changed file${files.length === 1 ? '' : 's'}` : 'Changes' }}
        <span v-if="files.length" class="counts"><span class="plus">+{{ totals.added }}</span><span class="minus">−{{ totals.removed }}</span></span>
      </h2>
      <button class="icon" :class="{ on: follow }" :aria-pressed="follow" title="Follow: jump to each file as it is written" @click="follow = !follow"><CircleDot :size="14" /></button>
      <span class="segmented-mini" role="group" aria-label="Diff layout">
        <button :aria-pressed="!sideBySide" @click="sideBySide = false">Inline</button>
        <button :aria-pressed="sideBySide" @click="sideBySide = true">Split</button>
      </span>
      <button class="icon" title="Refresh" @click="refreshStatus()"><RefreshCw :size="14" /></button>
    </div>

    <div v-if="!files.length" class="empty">
      <span class="glyph"><Check :size="22" /></span>
      <h2>Nothing changed yet</h2>
      <p>When you or an agent edit files in {{ status?.branch ?? 'this worktree' }}, each change shows up here the moment it is saved.</p>
    </div>

    <div v-else class="changes-split">
      <div class="changes-list">
        <template v-if="staged.length">
          <div class="section-title">Staged · {{ staged.length }}</div>
          <ul class="rows">
            <li v-for="file in staged" :key="'s' + file.path" class="row" :class="{ selected: selected === file.path }" @click="selected = file.path; follow = false">
              <span class="letter" :class="letter(file, true)">{{ letter(file, true) }}</span>
              <span class="name" :title="file.path">{{ short(file.path) }} <span class="muted small">{{ file.path.includes('/') ? file.path.slice(0, file.path.lastIndexOf('/')) : '' }}</span></span>
              <span class="hover-actions"><button class="icon" title="Unstage" @click.stop="git('unstage', [file.path])"><Minus :size="13" /></button></span>
              <span class="counts"><span class="plus">+{{ file.added ?? '·' }}</span><span class="minus">−{{ file.removed ?? '·' }}</span></span>
            </li>
          </ul>
        </template>
        <template v-if="unstaged.length">
          <div class="section-title">Changes · {{ unstaged.length }}</div>
          <ul class="rows">
            <li v-for="file in unstaged" :key="'u' + file.path" class="row" :class="{ selected: selected === file.path, flash: fileChanges.paths.includes(file.path) }" @click="selected = file.path; follow = false">
              <span class="letter" :class="letter(file, false)">{{ letter(file, false) }}</span>
              <span class="name" :title="file.path">{{ short(file.path) }} <span class="muted small">{{ file.path.includes('/') ? file.path.slice(0, file.path.lastIndexOf('/')) : '' }}</span></span>
              <span class="hover-actions">
                <button class="icon" title="Open in editor" @click.stop="openFile(file.path)"><FileCode2 :size="13" /></button>
                <button class="icon" title="Discard changes" @click.stop="discard(file)"><Undo2 :size="13" /></button>
                <button class="icon" title="Stage" @click.stop="git('stage', [file.path])"><Plus :size="13" /></button>
              </span>
              <span class="counts"><span class="plus">+{{ file.added ?? '·' }}</span><span class="minus">−{{ file.removed ?? '·' }}</span></span>
            </li>
          </ul>
        </template>
      </div>
      <div v-if="current" class="diff-head">
        <span class="name" :title="current.path">{{ current.original ? `${current.original} → ` : '' }}{{ current.path }}</span>
        <button class="ghost" @click="openFile(current.path)"><FileCode2 :size="13" /> Open</button>
      </div>
      <div ref="host" class="monaco-host" />
      <div class="commit-box">
        <textarea v-model="message" placeholder="Describe the change, then commit" aria-label="Commit message" @keydown.meta.enter="commit" @keydown.ctrl.enter="commit" />
        <div class="row-actions">
          <span class="muted small grow" style="flex:1">{{ staged.length ? `Commits the ${staged.length} staged file${staged.length === 1 ? '' : 's'}` : 'Commits every change' }} on {{ status?.branch ?? 'this branch' }}</span>
          <button class="primary-button" :disabled="committing || !message.trim()" @click="commit"><GitCommitHorizontal :size="14" /> Commit</button>
        </div>
      </div>
    </div>
  </section>
</template>

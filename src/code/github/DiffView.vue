<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ChevronRight, FileCode2, FileDiff, FileMinus, FilePlus, LoaderCircle } from '@lucide/vue'
import { openFile } from '../state'
import { parseDiff, type DiffFile } from './format'
import { ask } from './store'

/** The pull request's changes as GitHub has them, one card per file. When the
 *  pull request is the worktree on screen, lines open in the editor. */
const props = defineProps<{ number: number; local: boolean }>()

const text = ref<string | null>(null)
const failed = ref('')
watch(() => props.number, async number => {
  text.value = null
  failed.value = ''
  try { text.value = await ask<string>('code_gh_diff', { number }) } catch (e) { failed.value = String(e) }
}, { immediate: true })

const files = computed(() => text.value === null ? [] : parseDiff(text.value))
/** Big files start folded so a large pull request stays quick to open. */
const folded = reactive(new Map<string, boolean>())
const isFolded = (f: DiffFile) => folded.get(f.path) ?? (f.added + f.removed > 400 || files.value.length > 40)
const totals = computed(() => files.value.reduce((t, f) => ({ added: t.added + f.added, removed: t.removed + f.removed }), { added: 0, removed: 0 }))
const icon = (f: DiffFile) => f.status === 'added' ? FilePlus : f.status === 'deleted' ? FileMinus : FileDiff

function jump(path: string) {
  folded.set(path, false)
  document.getElementById(`gh-file-${path}`)?.scrollIntoView({ block: 'start', behavior: 'smooth' })
}
</script>

<template>
  <div class="gh-diff">
    <div v-if="failed" class="banner error"><p>{{ failed }}</p></div>
    <div v-else-if="text === null" class="gh-loading"><LoaderCircle :size="16" class="spin" /> Loading changes…</div>
    <p v-else-if="!files.length" class="muted gh-pad">No file changes.</p>
    <template v-else>
      <details class="gh-file-index">
        <summary>{{ files.length }} file{{ files.length === 1 ? '' : 's' }} changed <span class="counts"><span class="plus">+{{ totals.added }}</span><span class="minus">−{{ totals.removed }}</span></span></summary>
        <button v-for="f in files" :key="f.path" class="bare gh-index-row" @click="jump(f.path)">
          <component :is="icon(f)" :size="12" :class="f.status" /><span class="grow mono">{{ f.path }}</span>
          <span class="counts"><span class="plus">+{{ f.added }}</span><span class="minus">−{{ f.removed }}</span></span>
        </button>
      </details>
      <section v-for="f in files" :id="`gh-file-${f.path}`" :key="f.path" class="gh-file">
        <header class="gh-file-head" @click="folded.set(f.path, !isFolded(f))">
          <ChevronRight :size="13" class="chev" :class="{ open: !isFolded(f) }" />
          <component :is="icon(f)" :size="13" :class="f.status" />
          <span class="gh-file-path mono" :title="f.path">{{ f.oldPath ? `${f.oldPath} → ` : '' }}{{ f.path }}</span>
          <span class="counts"><span class="plus">+{{ f.added }}</span><span class="minus">−{{ f.removed }}</span></span>
          <button v-if="local && f.status !== 'deleted'" class="icon" title="Open in the editor" @click.stop="openFile(f.path)"><FileCode2 :size="13" /></button>
        </header>
        <div v-if="!isFolded(f)" class="gh-file-body">
          <p v-if="f.binary" class="muted gh-pad">Binary file</p>
          <p v-else-if="!f.hunks.length" class="muted gh-pad">{{ f.status === 'renamed' ? 'Renamed without changes' : 'No text changes' }}</p>
          <table v-else class="gh-lines">
            <tbody v-for="(hunk, h) in f.hunks" :key="h">
              <tr class="hunk"><td colspan="3"><span class="mono">@@</span> {{ hunk.header }}</td></tr>
              <tr v-for="(line, i) in hunk.lines" :key="i" :class="line.kind" @dblclick="local && line.new !== null && openFile(f.path, line.new)">
                <td class="ln">{{ line.old ?? '' }}</td>
                <td class="ln">{{ line.new ?? '' }}</td>
                <td class="code"><span class="sign">{{ line.kind === 'add' ? '+' : line.kind === 'del' ? '−' : ' ' }}</span>{{ line.text }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </template>
  </div>
</template>

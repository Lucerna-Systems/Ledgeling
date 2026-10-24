<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { Archive, ExternalLink, KeyRound, Link2, LoaderCircle, Plus, RefreshCw, Search, SquareKanban, Unlink, X } from '@lucide/vue'
import { root, tab } from '../state'
import { pendingTerminal } from '../terminalFocus'
import { ask as confirm } from '../../terminal/confirm'
import Avatar from './Avatar.vue'
import GhMarkdown from './GhMarkdown.vue'
import StateIcon from './StateIcon.vue'
import { labelStyle, projectColor } from './format'
import { board, boardAct, boardId, boardLoading, canBoards, follow, loadBoard, loadProjects, openOnGitHub, projects, projectsLoading, repo } from './store'
import type { Card } from './types'

onMounted(() => { if (canBoards.value && !projects.value) void loadProjects() })
watch(canBoards, can => { if (can && !projects.value) void loadProjects() })

const chosen = computed({ get: () => boardId.value, set: id => { boardId.value = id } })
const current = computed(() => projects.value?.find(p => p.id === boardId.value) ?? null)
const linked = computed(() => projects.value?.filter(p => p.linked) ?? [])
const others = computed(() => projects.value?.filter(p => !p.linked) ?? [])

/** The single-select field whose options are the columns; Status when there is one. */
const groupBy = ref<string | null>(null)
const field = computed(() => {
  const fields = board.value?.fields.filter(f => f.options.length) ?? []
  return fields.find(f => f.id === groupBy.value) ?? fields.find(f => f.name.toLowerCase() === 'status') ?? fields[0] ?? null
})
watch(() => board.value?.id, () => { groupBy.value = null })

const query = ref('')
const thisRepo = ref(false)
function visible(card: Card) {
  if (thisRepo.value && card.repo && card.repo !== repo.value?.nameWithOwner) return false
  const q = query.value.trim().toLowerCase()
  if (!q) return true
  return card.title.toLowerCase().includes(q) || `#${card.number}`.includes(q)
    || card.assignees.some(a => a.toLowerCase().includes(q)) || card.labels.some(l => l.name.toLowerCase().includes(q))
}

interface Column { id: string | null; name: string; color: string; description: string; cards: Card[] }
const columns = computed<Column[]>(() => {
  const f = field.value
  const cards = (board.value?.cards ?? []).filter(visible)
  if (!f) return [{ id: null, name: 'Items', color: 'GRAY', description: '', cards }]
  const known = new Set(f.options.map(o => o.id))
  const none: Column = { id: null, name: `No ${f.name.toLowerCase()}`, color: 'GRAY', description: '', cards: cards.filter(c => !known.has(c.values[f.id] ?? '')) }
  const rest = f.options.map(o => ({ id: o.id, name: o.name, color: o.color, description: o.description, cards: cards.filter(c => c.values[f.id] === o.id) }))
  return none.cards.length ? [none, ...rest] : rest
})

// ------------------------------------------------------------ drag and drop
const dragging = ref<string | null>(null)
const over = ref<string | null | undefined>(undefined)
const moving = reactive(new Set<string>())
function start(event: DragEvent, card: Card) {
  dragging.value = card.id
  event.dataTransfer?.setData('text/plain', card.id)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}
function end() { dragging.value = null; over.value = undefined }

/** Moves at once on screen, then tells GitHub; a refusal puts it back. */
async function moveTo(cardId: string, option: string | null) {
  const f = field.value
  const card = board.value?.cards.find(c => c.id === cardId)
  if (!f || !card || (card.values[f.id] ?? null) === option) return
  const before = card.values[f.id]
  const values = { ...card.values }
  if (option) values[f.id] = option
  else delete values[f.id]
  replace(cardId, { values })
  moving.add(cardId)
  try { await boardAct({ action: 'move', item: cardId, field: f.id, option }) } catch {
    const back = { ...card.values }
    if (before) back[f.id] = before
    replace(cardId, { values: back })
  } finally { moving.delete(cardId) }
}
function drop(option: string | null) {
  const id = dragging.value
  end()
  if (id) void moveTo(id, option)
}
function replace(id: string, patch: Partial<Card>) {
  if (!board.value) return
  board.value = { ...board.value, cards: board.value.cards.map(c => c.id === id ? { ...c, ...patch } : c) }
}

// ------------------------------------------------------------------ adding
const adding = ref<string | null | undefined>(undefined)
const newTitle = ref('')
const saving = ref(false)
/** `#12` adds that issue or pull request; anything else becomes a draft card. */
async function add(option: string | null) {
  const text = newTitle.value.trim()
  if (!text || saving.value) return
  saving.value = true
  try {
    const number = /^#?(\d+)$/.exec(text)?.[1]
    const id = number
      ? await boardAct({ action: 'addExisting', kind: 'issue', number: Number(number) })
      : await boardAct({ action: 'addDraft', title: text, body: '' })
    if (id && option && field.value) await boardAct({ action: 'move', item: id, field: field.value.id, option })
    newTitle.value = ''
    adding.value = undefined
    await loadBoard()
  } catch { /* shown above */ } finally { saving.value = false }
}

async function archive(card: Card) {
  if (!await confirm(`Archive “${card.title}” on this board? The issue itself is not changed.`, 'Archive')) return
  try {
    await boardAct({ action: 'archive', item: card.id })
    if (board.value) board.value = { ...board.value, cards: board.value.cards.filter(c => c.id !== card.id) }
  } catch { /* shown above */ }
}

async function link(on: boolean) {
  try { await boardAct({ action: on ? 'link' : 'unlink' }); await loadProjects() } catch { /* shown above */ }
}

const expanded = ref<string | null>(null)
function openCard(card: Card) {
  if (card.kind === 'draft') { expanded.value = expanded.value === card.id ? null : card.id; return }
  if (card.url) follow(card.url)
}

/** Board access needs the `project` scope, which `gh auth refresh` adds after a browser sign-in. */
function grant() {
  pendingTerminal.value = { worktree: root.value ?? '', kind: 'shell', title: 'GitHub board access', command: 'gh auth refresh --hostname github.com --scopes project' }
  tab.value = 'terminal'
}
</script>

<template>
  <div class="gh-boards">
    <div v-if="!canBoards" class="empty">
      <span class="glyph"><KeyRound :size="22" /></span>
      <h2>Boards need one more permission</h2>
      <p>Your GitHub sign-in can read code but not project boards. Grant it once in the browser, then come back.</p>
      <button class="primary-button" @click="grant"><KeyRound :size="14" /> Grant board access</button>
      <p class="muted small">Runs <code>gh auth refresh -s project</code> in a terminal you can watch.</p>
    </div>

    <template v-else>
      <div class="gh-board-bar">
        <SquareKanban :size="14" class="muted" />
        <select v-model="chosen" class="gh-board-pick" aria-label="Board" :disabled="projectsLoading || !projects?.length">
          <option v-if="!projects?.length" :value="null">{{ projectsLoading ? 'Loading boards…' : 'No boards' }}</option>
          <optgroup v-if="linked.length" :label="`Linked to ${repo?.name}`">
            <option v-for="p in linked" :key="p.id" :value="p.id">{{ p.title }}</option>
          </optgroup>
          <optgroup v-if="others.length" :label="`Other boards from ${repo?.owner}`">
            <option v-for="p in others" :key="p.id" :value="p.id">{{ p.title }}</option>
          </optgroup>
        </select>
        <select v-if="board && board.fields.filter(f => f.options.length).length > 1" :value="field?.id" aria-label="Columns from" class="gh-group-pick" @change="groupBy = ($event.target as HTMLSelectElement).value">
          <option v-for="f in board.fields.filter(f => f.options.length)" :key="f.id" :value="f.id">By {{ f.name }}</option>
        </select>
        <label class="gh-search gh-board-search">
          <Search :size="12" class="muted" />
          <input v-model="query" placeholder="Filter cards" aria-label="Filter cards" spellcheck="false">
          <button v-if="query" class="icon" title="Clear" @click="query = ''"><X :size="11" /></button>
        </label>
        <button class="gh-pill" :aria-pressed="thisRepo" title="Only cards from this repository" @click="thisRepo = !thisRepo">This repo</button>
        <span class="grow" />
        <button v-if="current && !current.linked" class="ghost" title="Show this board on the repository's Projects tab" @click="link(true)"><Link2 :size="13" /> Link to {{ repo?.name }}</button>
        <button v-if="current?.linked" class="icon" :title="`Unlink from ${repo?.name}`" @click="link(false)"><Unlink :size="13" /></button>
        <button class="icon" title="Reload" :disabled="boardLoading" @click="loadProjects(); loadBoard()"><RefreshCw :size="13" :class="{ spin: boardLoading }" /></button>
        <button v-if="board" class="icon" title="Open on GitHub" @click="openOnGitHub(board.url)"><ExternalLink :size="13" /></button>
      </div>

      <div v-if="projects && !projects.length" class="empty">
        <span class="glyph"><SquareKanban :size="22" /></span>
        <h2>No boards yet</h2>
        <p>Neither {{ repo?.nameWithOwner }} nor {{ repo?.owner }} has an open project board.</p>
        <button class="ghost" @click="repo && openOnGitHub(`${repo.url}/projects`)"><ExternalLink :size="13" /> Create one on GitHub</button>
      </div>
      <div v-else-if="!board" class="gh-loading"><LoaderCircle :size="16" class="spin" /> Loading board…</div>

      <div v-else class="gh-columns" @dragend="end">
        <section
          v-for="col in columns" :key="col.id ?? 'none'" class="gh-column" :class="{ over: dragging && over === col.id }"
          @dragover.prevent="over = col.id" @dragleave.self="over = undefined" @drop.prevent="drop(col.id)"
        >
          <header class="gh-column-head" :title="col.description">
            <span class="gh-col-dot" :style="{ background: projectColor(col.color) }" />
            <strong>{{ col.name }}</strong>
            <span class="gh-count">{{ col.cards.length }}</span>
            <span class="grow" />
            <button class="icon" :title="`Add a card to ${col.name}`" @click="adding = col.id; newTitle = ''"><Plus :size="13" /></button>
          </header>
          <div class="gh-cards">
            <form v-if="adding === col.id" class="gh-add-card" @submit.prevent="add(col.id)">
              <input v-model="newTitle" v-focus placeholder="Title, or #12 to add an issue" aria-label="New card" @keydown.esc="adding = undefined">
              <div class="gh-add-actions">
                <button type="button" class="ghost" @click="adding = undefined">Cancel</button>
                <button type="submit" class="primary-button" :disabled="!newTitle.trim() || saving"><LoaderCircle v-if="saving" :size="12" class="spin" /> Add</button>
              </div>
            </form>
            <article
              v-for="card in col.cards" :key="card.id" class="gh-card" :class="{ dragging: dragging === card.id, moving: moving.has(card.id), draft: card.kind === 'draft' }"
              draggable="true" tabindex="0" @dragstart="start($event, card)" @click="openCard(card)" @keydown.enter="openCard(card)"
            >
              <div class="gh-card-top">
                <StateIcon :kind="card.kind" :state="card.state" :draft="card.draftPr" :size="13" />
                <span class="muted small mono">{{ card.kind === 'draft' ? 'Draft' : card.repo && card.repo !== repo?.nameWithOwner ? `${card.repo.split('/')[1]}#${card.number}` : card.number ? `#${card.number}` : '' }}</span>
                <span class="grow" />
                <LoaderCircle v-if="moving.has(card.id)" :size="11" class="spin muted" />
                <button class="icon gh-card-menu" title="Archive from this board" @click.stop="archive(card)"><Archive :size="11" /></button>
              </div>
              <p class="gh-card-title">{{ card.title }}</p>
              <div v-if="card.labels.length || card.assignees.length" class="gh-card-foot">
                <span v-for="l in card.labels.slice(0, 3)" :key="l.name" class="gh-label" :style="labelStyle(l.color)">{{ l.name }}</span>
                <span class="grow" />
                <span class="gh-stack"><Avatar v-for="a in card.assignees.slice(0, 3)" :key="a" :login="a" :size="16" /></span>
              </div>
              <GhMarkdown v-if="expanded === card.id && card.body" class="gh-card-body" :text="card.body" :repo-url="repo?.url" @link="follow" @click.stop />
            </article>
            <p v-if="!col.cards.length && adding !== col.id" class="gh-column-empty">Drop cards here</p>
          </div>
        </section>
      </div>
      <p v-if="board?.truncated" class="muted small gh-pad">This board has more than 500 cards; the first 500 are shown.</p>
    </template>
  </div>
</template>

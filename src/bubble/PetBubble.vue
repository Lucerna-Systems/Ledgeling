<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Folder, X } from '@lucide/vue'
import MarkdownResult from '../assistant/MarkdownResult.vue'
import Dropdown from '../components/Dropdown.vue'
// xterm is only fetched when the Terminal mode is first opened.
const BubbleTerminal = defineAsyncComponent(() => import('../terminal/BubbleTerminal.vue'))
import { applyCachedTheme } from '../theme'
import {
  ACTIONS, NOTCH, blocker, bubblePath, connected, isTask, shortPath, startEvent, taskView,
  type BubbleShow, type BubbleUpdate, type Draft, type TaskView,
} from './logic'

/** The speech bubble a right-click on a pet opens. Like the pet overlay it
 *  decides nothing: the backend says which pet it is for, where it stands and
 *  what a task is doing, and turns what the user asks for into a request. */
const show = ref<BubbleShow | null>(null)
const draft = reactive<Draft>({ mode: 'actions', action: 0, prompt: '', allowCloud: false, allowWeb: false })
const task = ref<TaskView | null>(null)
/** The last finished task, sent along so a follow-up has its context. */
const parentId = ref<number | null>(null)
/** A plain message from the backend: a saved model, a bound folder, an error. */
const note = ref('')
/** Approve and Reject wait for the next update once one has been clicked. */
const answering = ref(false)
const models = reactive({ state: 'link' as 'link' | 'loading' | 'picking', list: [] as string[], index: 0 })
const tail = reactive({ x: 60, up: false })
const visible = ref(false)
/** What the content measures: where the bubble is going. */
const natural = reactive({ width: 370, height: 300 })
/** What is drawn this frame: it catches up to `natural` over a short tween, so
 *  the bubble grows and shrinks instead of jumping. */
const shown = reactive({ width: 370, height: 300 })
const root = ref<HTMLElement>()
const prompt = ref<HTMLTextAreaElement>()
const off: UnlistenFn[] = []
let closing: ReturnType<typeof setTimeout> | undefined
let reported = 0
let reportedWidth = 0
let observer: ResizeObserver | undefined
let tween = 0
let disposed = false

const mac = /Mac/i.test(navigator.platform || navigator.userAgent)
/** Window margin around the drawn bubble; matches BUBBLE_MARGIN in pet_ui.rs. */
const MARGIN = 12

const ask = computed(() => draft.mode === 'ask')
const terminal = computed(() => draft.mode === 'terminal')
const MODES = ['actions', 'ask', 'terminal'] as const
/** Content width: a terminal wants more room than a task form. Matches the backend's clamp. */
const contentWidth = computed(() => terminal.value ? 560 : 370)
const isConnected = computed(() => !!show.value && connected(show.value.runtime))
const hint = computed(() => show.value ? blocker(show.value, draft) : null)
const running = computed(() => !!task.value && !task.value.done)
const canSend = computed(() => !hint.value && !running.value)
const path = computed(() => bubblePath(shown.width, shown.height, tail.x, tail.up))
const approvalLines = computed(() => task.value?.approval?.summary.items.filter(Boolean) ?? [])

function call(event: Record<string, unknown>) {
  return invoke('pet_bubble', { event }).catch(error => { note.value = String(error) })
}

function reducedMotion() {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false
}

function place(value: { tailX: number | null; tailUp: boolean }) {
  if (typeof value.tailX === 'number') tail.x = value.tailX
  tail.up = value.tailUp
}

async function open(payload: BubbleShow) {
  clearTimeout(closing)
  closing = undefined
  applyCachedTheme()
  if (show.value?.petId !== payload.petId) {
    // A different pet is a different conversation.
    draft.prompt = ''
    task.value = null
    parentId.value = null
    note.value = ''
  }
  show.value = payload
  place(payload)
  // Every opening starts the window at the ordinary width, so the size has to
  // be reported again even if it is unchanged since the last time: reopening
  // in Terminal mode would otherwise stay cropped to the narrow window.
  reported = 0
  reportedWidth = 0
  // A fresh open never starts mid-pick, whatever the last pet left behind.
  models.state = 'link'
  answering.value = false
  visible.value = true
  await nextTick()
  prompt.value?.focus()
  // A fresh opening appears at its size; it does not grow from the last one.
  report(true)
}

function update(value: BubbleUpdate) {
  const current = show.value
  if (!current) return
  if (isTask(value)) {
    if (value.request.petId !== current.petId) return
    const view = taskView(value)
    task.value = view
    note.value = ''
    answering.value = false
    if (view.done) parentId.value = view.id
    return
  }
  if (value.petId !== undefined && value.petId !== current.petId) return
  if (value.folder !== undefined) current.folder = value.folder
  if (value.model !== undefined) current.model = value.model
  if (value.models !== undefined) {
    // Only a list with entries is worth swapping the link for the picker.
    if (value.models.length) {
      models.list = value.models
      models.index = Math.max(0, value.models.indexOf(current.model))
      models.state = 'picking'
    } else {
      models.state = 'link'
    }
  }
  if (value.message !== undefined) {
    if (models.state === 'loading') models.state = 'link'
    note.value = value.message
  }
}

/** Fade out, then report the close; the backend hides the window and lets the
 *  pet go. Work carries on either way. */
function dismiss() {
  if (!show.value || closing) return
  visible.value = false
  const { session, petId } = show.value
  closing = setTimeout(() => {
    closing = undefined
    void call({ type: 'closed', session, petId })
  }, reducedMotion() ? 0 : 140)
}

function send() {
  if (!show.value || !canSend.value) return
  note.value = ''
  void call(startEvent(show.value.petId, draft, parentId.value))
}

function answer(accept: boolean) {
  if (!task.value?.approval) return
  answering.value = true
  void call({ type: 'approve', id: task.value.id, approvalId: task.value.approval.id, accept })
}

function control(type: 'cancel' | 'undo') {
  if (task.value) void call({ type, id: task.value.id })
}

function openTerminal() {
  if (!show.value) return
  void call({ type: 'terminal', petId: show.value.petId })
  dismiss()
}

function codeHere() {
  if (!show.value) return
  void call({ type: 'code', petId: show.value.petId })
  dismiss()
}

function settings() {
  if (!show.value) return
  void call({ type: 'settings', petId: show.value.petId })
  dismiss()
}

function chooseFolder() {
  if (show.value) void call({ type: 'choose', petId: show.value.petId })
}

function pickModel() {
  if (!show.value || models.state !== 'link') return
  // Disabled while the list loads: a second click would queue a second fetch
  // behind the first for no gain. Any reply re-enables it.
  models.state = 'loading'
  void call({ type: 'models', petId: show.value.petId })
}

function chooseModel(index: number) {
  const model = models.list[index]
  models.state = 'link'
  if (show.value && model) void call({ type: 'model', petId: show.value.petId, model })
}

function openLink(url: string) {
  void call({ type: 'open', url })
}

function keydown(event: KeyboardEvent) {
  // Escape belongs to the program in the terminal (Claude Code interrupts on
  // it); the close button and clicking away still dismiss the bubble.
  if (event.key === 'Escape' && !(event.target as HTMLElement | null)?.closest?.('.bubble-term')) { event.preventDefault(); dismiss() }
}

function promptKey(event: KeyboardEvent) {
  if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) { event.preventDefault(); send() }
}

/** Tells the backend how big the window must be. */
function sendSize(width: number, height: number) {
  const total = Math.ceil(height + MARGIN * 2)
  if (Math.abs(total - reported) < 1 && width === reportedWidth) return
  reported = total
  reportedWidth = width
  void call({ type: 'size', height: total, width })
}

const DURATION = 240
const ease = (t: number) => 1 - (1 - t) ** 3

/** Draw the bubble at its new size by moving toward it. The window is the
 *  bigger of the two sizes while that happens, so nothing is cropped on the
 *  way, and settles to the new size at the end. */
function morph(to: { width: number; height: number }, snap: boolean) {
  cancelAnimationFrame(tween)
  if (snap || reducedMotion()) {
    shown.width = to.width
    shown.height = to.height
    sendSize(to.width, to.height)
    return
  }
  const from = { width: shown.width, height: shown.height }
  sendSize(Math.max(from.width, to.width), Math.max(from.height, to.height))
  const start = performance.now()
  const step = (now: number) => {
    const t = Math.min(1, (now - start) / DURATION)
    const k = ease(t)
    shown.width = from.width + (to.width - from.width) * k
    shown.height = from.height + (to.height - from.height) * k
    if (t < 1) { tween = requestAnimationFrame(step); return }
    sendSize(to.width, to.height)
  }
  tween = requestAnimationFrame(step)
}

/** The window is exactly as big as the bubble; the backend sizes and places
 *  it so the tail stays on the pet as the content grows or shrinks. */
function report(snap = false) {
  const element = root.value
  if (!element || !show.value) return
  const width = element.offsetWidth || natural.width
  const height = element.offsetHeight || natural.height
  if (!snap && width === natural.width && height === natural.height) return
  natural.width = width
  natural.height = height
  morph(natural, snap)
}

// Rows appearing and disappearing change the height before the observer has
// a chance to fire in a hidden window.
watch([() => draft.mode, hint, task, note, () => models.state], () => { void nextTick(() => report()) })
watch(terminal, on => { if (!on) void nextTick(() => prompt.value?.focus()) })

async function subscribe<T>(event: string, callback: (value: T) => void) {
  const stop = await listen<T>(event, e => callback(e.payload))
  if (disposed) stop(); else off.push(stop)
}

onMounted(async () => {
  // The bubble's root only exists while it is open, and its height changes
  // when content arrives late (the terminal loads on demand), so watch it
  // whenever it appears rather than once at mount.
  if (typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(() => report())
    watch(root, (element, previous) => {
      if (previous) observer?.unobserve(previous)
      if (element) observer?.observe(element)
    }, { immediate: true, flush: 'post' })
  }
  document.addEventListener('keydown', keydown)
  await subscribe<BubbleShow>('bubble:show', payload => void open(payload))
  await subscribe<BubbleUpdate>('bubble:update', update)
  await subscribe<{ tailX: number; tailUp: boolean }>('bubble:place', place)
  await subscribe<number>('bubble:dismiss', session => { if (session === show.value?.session) dismiss() })
  // The opening may have been sent before this page could listen.
  await invoke('pet_bubble', { event: { type: 'ready' } }).catch(() => {})
})
onUnmounted(() => {
  disposed = true
  off.forEach(stop => stop())
  observer?.disconnect()
  cancelAnimationFrame(tween)
  document.removeEventListener('keydown', keydown)
  clearTimeout(closing)
})
</script>

<template>
  <div class="bubble-page" :class="{ visible, up: tail.up }">
    <div v-if="show" class="bubble-frame" :style="{ width: `${shown.width}px`, height: `${shown.height}px` }">
    <svg class="bubble-shape" :viewBox="`0 0 ${shown.width} ${shown.height}`" aria-hidden="true"><path :d="path" /></svg>
    <div class="bubble-clip">
    <section
      ref="root"
      class="bubble"
      :class="{ 'tail-up': tail.up }"
      role="dialog"
      :aria-label="`Ask ${show.name}`"
      :style="{ width: `${contentWidth}px`, paddingTop: `${(tail.up ? NOTCH : 0) + 16}px`, paddingBottom: `${(tail.up ? 0 : NOTCH) + 18}px` }"
    >
      <header class="bubble-head">
        <h1>Ask {{ show.name }}</h1>
        <button type="button" class="icon-button" aria-label="Close" @click="dismiss"><X :size="15" /></button>
      </header>

      <button type="button" class="folder-row" :title="show.folder || undefined" @click="chooseFolder">
        <Folder :size="15" class="folder-icon" aria-hidden="true" />
        <span class="folder-path" :class="{ empty: !show.folder }">{{ show.folder ? shortPath(show.folder) : (mac ? 'Choose a folder, or drop me into one' : 'Choose a folder') }}</span>
        <span class="folder-action">{{ show.folder ? 'Change' : 'Choose' }}</span>
        <span class="sr-only">{{ show.folder ? `Working folder ${show.folder}. Choose another.` : 'Choose a working folder' }}</span>
      </button>

      <div class="mode-row">
        <!-- Two choices, because there are only two: run an action, or ask the
             model. Which loop asking runs follows from the connection. -->
        <div class="bubble-mode" role="radiogroup" aria-label="Mode">
          <button v-for="m in MODES" :key="m" type="button" role="radio" :aria-checked="draft.mode === m" :class="{ on: draft.mode === m }" @click="draft.mode = m">{{ m === 'ask' ? 'Ask' : m === 'terminal' ? 'Terminal' : 'Actions' }}</button>
          <span class="pill" :style="{ transform: `translateX(${MODES.indexOf(draft.mode) * 100}%)` }" aria-hidden="true" />
        </div>
        <Dropdown v-if="!ask && !terminal" v-model="draft.action" class="action-select" label="Action" :options="ACTIONS.map(a => a.label)" />
      </div>

      <BubbleTerminal v-if="terminal" :pet-id="show.petId" @pop-out="openTerminal" />

      <template v-else>
        <p v-if="hint" class="hint" role="status">{{ hint }}</p>

        <textarea
          ref="prompt"
          v-model="draft.prompt"
          class="prompt"
          :placeholder="ask ? 'Ask anything…' : 'Add details (optional)…'"
          :aria-label="ask ? 'Question' : 'Details'"
          spellcheck="false"
          autocapitalize="off"
          autocomplete="off"
          @keydown="promptKey"
        />

        <label v-if="ask" class="check"><input v-model="draft.allowWeb" type="checkbox">Allow web research</label>

        <div v-if="isConnected && show.runtime !== 'apple'" class="model-row">
          <Dropdown v-if="models.state === 'picking'" :model-value="models.index" class="model-select" label="Model" :options="models.list" @update:model-value="chooseModel" />
          <template v-else>
            <span v-if="show.model" class="model-name" :title="show.model">{{ show.model }}</span>
            <button type="button" class="link" :disabled="models.state === 'loading'" @click="pickModel">{{ models.state === 'loading' ? 'Loading models…' : 'Select another model' }}</button>
          </template>
        </div>

        <div class="actions">
          <button type="button" class="secondary" @click="settings">AI providers</button>
          <button v-if="show.developer && show.folder" type="button" class="secondary" title="Open this folder in coding mode, with this pet working in it" @click="codeHere">Code here</button>
          <button type="button" class="primary" :disabled="!canSend" @click="send">{{ ask ? 'Send request' : 'Run action' }}</button>
        </div>

        <div v-if="note || task" class="output" :class="{ mono: !note && task?.approval?.summary.monospace }" aria-live="polite">
          <p v-if="note">{{ note }}</p>
          <template v-else-if="task?.approval">
            <p class="headline">{{ task.approval.summary.headline || task.approval.title || 'This task needs your approval.' }}</p>
            <ul v-if="approvalLines.length || task.approval.summary.more">
              <li v-for="(item, index) in approvalLines" :key="index">{{ item }}</li>
              <li v-if="task.approval.summary.more > 0" class="more">+ {{ task.approval.summary.more }} more</li>
            </ul>
            <p v-if="task.approval.summary.note" class="approval-note">{{ task.approval.summary.note }}</p>
          </template>
          <template v-else-if="task">
            <p v-if="task.progress" class="progress" role="status">{{ task.progress }}</p>
            <p v-if="task.error" class="problem" role="alert">{{ task.error }}</p>
            <MarkdownResult v-if="task.result" :text="task.result" @open="openLink" />
          </template>
        </div>

        <div v-if="task && (task.approval || task.undoable || !task.done)" class="actions start">
          <template v-if="task.approval">
            <button type="button" class="primary" :disabled="answering" @click="answer(true)">{{ task.approval.summary.accept || 'Approve' }}</button>
            <button type="button" class="secondary" :disabled="answering" @click="answer(false)">Reject</button>
          </template>
          <button v-if="task.undoable" type="button" class="secondary" @click="control('undo')">Undo</button>
          <button v-if="!task.done" type="button" class="secondary" :disabled="task.status === 'cancelling'" @click="control('cancel')">Cancel task</button>
        </div>
      </template>
    </section>
    </div>
    </div>
  </div>
</template>

<style>
html:has(.bubble-page), html:has(.bubble-page) body { overflow: hidden; }
.bubble-page {
  box-sizing: border-box; width: 100vw; height: 100vh; padding: 12px; display: flex; flex-direction: column; justify-content: flex-end; opacity: 0; transition: opacity 140ms ease-in;
  font-size: 12.5px; line-height: 1.45; color: var(--ink);
}
.bubble-page.up { justify-content: flex-start; }
.bubble-page.visible { opacity: 1; transition: opacity 180ms ease-out; }
/* The drawn bubble: its shape and a clip that the content slides inside while it resizes. */
.bubble-frame { position: relative; flex: none; }
.bubble-clip { position: absolute; inset: 0; overflow: hidden; }
.bubble-page .bubble { position: absolute; left: 0; bottom: 0; }
.bubble-page.up .bubble { top: 0; bottom: auto; }
.bubble { position: relative; padding: 16px 20px 18px; display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; }
.bubble > * { position: relative; }
.bubble-shape { position: absolute !important; inset: 0; width: 100%; height: 100%; overflow: visible; filter: drop-shadow(0 3px 7px rgb(20 30 25 / .18)); }
.bubble-shape path { fill: color-mix(in srgb, var(--surface) 97%, transparent); stroke: var(--line-strong); stroke-width: 1; }
.bubble button { border: 0; background: none; cursor: pointer; font: inherit; color: inherit; }
.bubble button:disabled { cursor: default; opacity: .45; }
.bubble-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.bubble-head h1 { margin: 0; font-size: 16px; font-weight: 600; letter-spacing: -.2px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.icon-button { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 50%; color: var(--muted); }
.icon-button:hover { background: color-mix(in srgb, var(--muted) 14%, transparent); color: var(--ink); }
.folder-row {
  display: flex; align-items: center; gap: 9px; width: 100%; min-height: 34px; padding: 0 14px; border-radius: 11px !important;
  border: 1px solid var(--line) !important; background: var(--soft) !important; text-align: left; transition: border-color 120ms, background-color 120ms;
}
.folder-row:hover { border-color: var(--line-strong) !important; }
.folder-icon { color: var(--accent); }
.folder-path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.folder-path.empty { color: var(--muted); }
.folder-action { color: var(--accent); font-weight: 500; font-size: 11.5px; }
.mode-row { display: flex; gap: 12px; align-items: center; }
.bubble-mode { position: relative; display: grid; grid-template-columns: repeat(3, 1fr); grid-template-rows: 100%; flex: none; width: 186px; height: 28px; padding: 2.5px; border-radius: 14px; background: var(--soft); border: 1px solid var(--line); }
.bubble .bubble-mode button { position: relative; z-index: 1; height: 100%; padding: 0; line-height: 1; border-radius: 12px; color: var(--muted); font-size: 12px; transition: color 180ms; }
.bubble .bubble-mode button.on { color: var(--ink); font-weight: 500; }
.bubble-mode .pill { position: absolute; top: 2.5px; bottom: 2.5px; left: 2.5px; width: calc(100% / 3 - 1.7px); border-radius: 12px; background: var(--surface); box-shadow: 0 1px 2px rgb(0 0 0 / .08); transition: transform 220ms cubic-bezier(.2, .8, .2, 1); }
.action-select { flex: 1; min-width: 0; }
/* The bubble's dropdowns are a touch smaller and rounder than the window's. */
.bubble .dropdown-button { height: 28px; border-radius: 14px; font-size: 12px; }
.bubble .dropdown-list { max-height: 164px; }
.bubble .dropdown-list li { padding: 5px 10px; font-size: 12px; }
.hint { margin: -2px 0 0; font-size: 11.5px; color: var(--status-waiting-ink); }
.prompt {
  display: block; width: 100%; height: 84px; resize: none; padding: 10px 12px; border-radius: 14px; font: inherit; line-height: 1.5;
  border: 1px solid var(--line); background: var(--surface); color: var(--ink); outline: none;
}
.prompt:focus { border-color: var(--accent); }
.bubble .prompt:focus-visible { outline: none; }
.prompt::placeholder { color: var(--placeholder); }
.check { display: flex; align-items: center; gap: 8px; font-size: 11.5px; color: var(--body-ink); }
.check input { accent-color: var(--accent); width: 15px; height: 15px; margin: 0; }
.model-row { display: flex; align-items: center; gap: 8px; min-height: 18px; font-size: 11px; }
.model-name { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 55%; }
.model-select { flex: 1; }
.bubble .link { padding: 0; color: var(--accent); font-weight: 500; font-size: 11px; }
.bubble .link:not(:disabled):hover { text-decoration: underline; }
.actions { display: flex; justify-content: space-between; gap: 10px; }
.actions.start { justify-content: flex-start; }
.actions button { min-width: 92px; height: 32px; padding: 0 16px; border-radius: 16px !important; font-size: 12px; font-weight: 500; }
.actions .primary { background: var(--accent-gradient) !important; color: var(--on-accent); }
.actions .primary:not(:disabled):hover { filter: brightness(1.07); }
.actions .secondary { min-width: 0; padding: 0 12px; background: var(--soft) !important; border: 1px solid var(--line) !important; }
.actions .secondary:not(:disabled):hover { border-color: var(--line-strong) !important; }
.output {
  max-height: 168px; overflow-y: auto; padding: 9px 12px; border-radius: 14px; border: 1px solid var(--line); background: var(--soft);
  font-size: 12px; color: var(--body-ink); overflow-wrap: anywhere;
}
.output p { margin: 0 0 6px; }
.output p:last-child { margin-bottom: 0; }
.output ul { margin: 4px 0 6px; padding-left: 18px; }
.output.mono ul { font: 11px/1.55 ui-monospace, Menlo, Consolas, monospace; }
.output .headline { font-weight: 500; color: var(--ink); }
.output .more { list-style: none; margin-left: -18px; color: var(--muted); }
.output .progress { color: var(--muted); }
.output .problem { color: var(--problem-ink); }
.output .markdown-body { font-size: 12px; line-height: 1.6; }
.output .markdown-body :is(h1, h2, h3, h4, h5, h6) { font-size: 13px; margin: 10px 0 4px; }
.output .markdown-body pre { white-space: pre-wrap; font-size: 11px; }
.sr-only { position: absolute !important; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
@keyframes bubble-fade { from { opacity: 0; } }
@media (prefers-reduced-motion: reduce) {
  .bubble-page, .bubble-page.visible, .bubble-mode .pill, .bubble .bubble-mode button, .folder-row { transition: none; }
}
</style>

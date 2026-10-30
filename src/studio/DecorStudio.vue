<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { AlertCircle, Check, Eraser, Pencil, Plus, Trash2 } from '@lucide/vue'
import { call, native } from '../state'
import { decorPayload, decorProblems, MAX_SIDE, newProp, resize, starterDecor, SWAYS, type DecorDraft } from './decorModel'

/** Decor mode: draw props for biomes and save them as a decor pack. Rust
 *  validates the pack with the same loader as a hand-written one. */
const draft = ref<DecorDraft>(starterDecor())
const selected = ref(0)
/** Pixels are invisible to Vue; this is bumped after every stroke. */
const edits = ref(0)
const prop = computed(() => draft.value.props[selected.value])
const issues = computed(() => (edits.value, decorProblems(draft.value)))
const saving = ref(false)
const saved = ref('')
const colour = ref('#5c9648')
const erasing = ref(false)
const canvas = ref<HTMLCanvasElement>()

const zoom = computed(() => prop.value ? Math.max(3, Math.floor(320 / Math.max(prop.value.width, prop.value.height))) : 8)

function addProp() {
  draft.value.props.push(newProp(draft.value.props.map(p => p.name)))
  selected.value = draft.value.props.length - 1
}
function removeProp(index: number) {
  draft.value.props.splice(index, 1)
  selected.value = Math.max(0, Math.min(selected.value, draft.value.props.length - 1))
}
function setSize(axis: 'width' | 'height', event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  const current = prop.value
  draft.value.props[selected.value] = resize(current, axis === 'width' ? value : current.width, axis === 'height' ? value : current.height)
  edits.value++
}
function togglePerch() {
  prop.value.perch = prop.value.perch ? null : { y: 0.1, from: 0.15, to: 0.85 }
}

function rgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}
let painting = false
function paintAt(event: PointerEvent) {
  const p = prop.value
  const box = canvas.value?.getBoundingClientRect()
  if (!p || !box) return
  const x = Math.floor((event.clientX - box.left) / zoom.value)
  const y = Math.floor((event.clientY - box.top) / zoom.value)
  if (x < 0 || y < 0 || x >= p.width || y >= p.height) return
  const [r, g, b] = rgb(colour.value)
  p.pixels.set(erasing.value ? [0, 0, 0, 0] : [r, g, b, 255], (y * p.width + x) * 4)
  edits.value++
}
function down(event: PointerEvent) {
  if (event.button !== 0) return
  painting = true
  ;(event.target as HTMLElement).setPointerCapture?.(event.pointerId)
  paintAt(event)
}
function move(event: PointerEvent) { if (painting) paintAt(event) }
function up() { painting = false }
function clear() { prop.value.pixels.fill(0); edits.value++ }

function draw() {
  const p = prop.value
  const ctx = canvas.value?.getContext('2d')
  if (!p || !ctx) return
  const z = zoom.value
  canvas.value!.width = p.width * z
  canvas.value!.height = p.height * z
  for (let y = 0; y < p.height; y++) {
    for (let x = 0; x < p.width; x++) {
      const i = (y * p.width + x) * 4
      ctx.fillStyle = (x + y) % 2 ? '#e9e8e0' : '#f3f2ec'
      ctx.fillRect(x * z, y * z, z, z)
      if (p.pixels[i + 3]) {
        ctx.fillStyle = `rgba(${p.pixels[i]}, ${p.pixels[i + 1]}, ${p.pixels[i + 2]}, ${p.pixels[i + 3] / 255})`
        ctx.fillRect(x * z, y * z, z, z)
      }
    }
  }
  if (p.perch) {
    ctx.strokeStyle = '#5b5bd6'
    ctx.lineWidth = 2
    ctx.setLineDash([6, 4])
    ctx.beginPath()
    ctx.moveTo(p.perch.from * p.width * z, p.perch.y * p.height * z)
    ctx.lineTo(p.perch.to * p.width * z, p.perch.y * p.height * z)
    ctx.stroke()
  }
}
watch([edits, selected, zoom, () => prop.value?.perch && { ...prop.value.perch }], draw, { flush: 'post' })
onMounted(draw)

async function save() {
  if (issues.value.length || saving.value) return
  saving.value = true
  saved.value = ''
  try {
    const id = await call<string>('save_decor_pack', decorPayload(draft.value))
    if (id) saved.value = draft.value.name.trim()
  } finally { saving.value = false }
}
</script>

<template>
  <div class="decor-studio">
    <header class="studio-head">
      <div class="studio-identity">
        <input v-model="draft.name" class="studio-name" aria-label="Decor pack name" placeholder="Name your decor pack" maxlength="40" spellcheck="false">
        <div class="studio-meta">
          <input v-model="draft.author" aria-label="Made by" placeholder="Made by (optional)" maxlength="40">
        </div>
      </div>
      <div class="studio-actions">
        <button type="button" class="primary-button" :disabled="!!issues.length || saving || !native" @click="save">{{ saving ? 'Saving…' : 'Save decor' }}</button>
      </div>
    </header>

    <div v-if="saved" class="studio-saved" role="status">
      <Check :size="16" /> <span><strong>{{ saved }}</strong> is saved. Its props are in the tray when you decorate a window.</span>
    </div>
    <div v-else-if="issues.length" class="studio-issues">
      <AlertCircle :size="15" />
      <span>Before saving: {{ issues[0] }}<template v-if="issues.length > 1"> <em>(+{{ issues.length - 1 }} more)</em></template></span>
    </div>

    <div class="decor-body">
      <aside class="decor-props" aria-label="Props">
        <button v-for="(p, i) in draft.props" :key="i" type="button" class="decor-prop" :aria-pressed="i === selected" @click="selected = i">{{ p.name }}</button>
        <button type="button" class="text-link" @click="addProp"><Plus :size="14" /> Add prop</button>
      </aside>

      <section v-if="prop" class="decor-editor">
        <div class="decor-fields">
          <label>Name <input v-model="prop.name" aria-label="Prop name" maxlength="32" spellcheck="false" @input="edits++"></label>
          <label>Width <input type="number" min="4" :max="MAX_SIDE" :value="prop.width" aria-label="Width" @change="setSize('width', $event)"></label>
          <label>Height <input type="number" min="4" :max="MAX_SIDE" :value="prop.height" aria-label="Height" @change="setSize('height', $event)"></label>
          <label>Sway
            <select v-model="prop.sway" aria-label="Sway">
              <option v-for="s in SWAYS" :key="s.value" :value="s.value" :title="s.blurb">{{ s.label }}</option>
            </select>
          </label>
          <label class="decor-check"><input type="checkbox" :checked="!!prop.perch" aria-label="Pets can sit on it" @change="togglePerch"> Pets can sit on it</label>
          <button type="button" class="text-link" :disabled="draft.props.length < 2" @click="removeProp(selected)"><Trash2 :size="13" /> Remove prop</button>
        </div>
        <div v-if="prop.perch" class="decor-fields">
          <label>Ledge height <input v-model.number="prop.perch.y" type="range" min="0" max="1" step="0.01" aria-label="Ledge height"></label>
          <label>From <input v-model.number="prop.perch.from" type="range" min="0" max="1" step="0.01" aria-label="Ledge from" @input="edits++"></label>
          <label>To <input v-model.number="prop.perch.to" type="range" min="0" max="1" step="0.01" aria-label="Ledge to" @input="edits++"></label>
        </div>
        <div class="decor-tools">
          <input v-model="colour" type="color" aria-label="Colour">
          <button type="button" :aria-pressed="!erasing" title="Draw" @click="erasing = false"><Pencil :size="15" /></button>
          <button type="button" :aria-pressed="erasing" title="Erase" @click="erasing = true"><Eraser :size="15" /></button>
          <button type="button" class="text-link" @click="clear">Clear</button>
          <span class="decor-note">The bottom row stands on the window's edge. The dashed line is the ledge.</span>
        </div>
        <canvas ref="canvas" class="decor-canvas" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="up" />
      </section>
    </div>
  </div>
</template>

<style scoped>
.decor-body { display: flex; gap: 16px; align-items: flex-start; }
.decor-props { display: flex; flex-direction: column; gap: 4px; min-width: 120px; }
.decor-prop { text-align: left; }
.decor-prop[aria-pressed="true"] { outline: 2px solid var(--accent); }
.decor-editor { display: flex; flex-direction: column; gap: 10px; }
.decor-fields, .decor-tools { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }
.decor-fields label { display: flex; gap: 6px; align-items: center; font-size: 12.5px; color: var(--muted); }
.decor-fields input[type="number"] { width: 56px; }
.decor-tools button[aria-pressed="true"] { outline: 2px solid var(--accent); }
.decor-note { color: var(--muted); font-size: 12px; }
.decor-canvas { image-rendering: pixelated; cursor: crosshair; border-radius: 6px; touch-action: none; align-self: flex-start; }
</style>

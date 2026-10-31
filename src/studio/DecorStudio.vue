<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowLeft, Plus, Trash2 } from '@lucide/vue'
import { call, native } from '../state'
import { cloneDecor, decorId, decorPayload, decorProblems, MAX_SIDE, newProp, PROP_ANIMATIONS, resize, starterDecor, SWAYS, type DecorDraft, type PropAnimation } from './decorModel'
import { uniqueId } from './model'
import SpriteEditor from './SpriteEditor.vue'
import StudioShell from './StudioShell.vue'
import { thumbnailer } from './thumbs'

/** Draws props for biomes and saves them as a decor pack. Rust validates the
 *  pack with the same loader as a hand-written one. The Decor page hosts it:
 *  `initial` opens an existing pack, either as a copy or to change in place. */
const props = withDefaults(defineProps<{ initial?: DecorDraft; mode?: 'new' | 'copy' | 'in_place' }>(), { initial: undefined, mode: 'new' })
const emit = defineEmits<{ saved: [id: string]; close: [] }>()

/** The id of the pack a copy was made from, which the copy must not reuse. */
const original = props.mode === 'copy' ? props.initial?.id : undefined
function opening(): DecorDraft {
  if (!props.initial) return starterDecor()
  const draft = cloneDecor(props.initial)
  if (props.mode === 'in_place') return draft
  // Anything but an in-place edit is a new pack, whose id comes from its name.
  delete draft.id
  if (original && !draft.name.endsWith(' copy')) draft.name = `${draft.name} copy`
  return draft
}
const draft = ref<DecorDraft>(opening())
const selected = ref(0)
/** Pixels are invisible to Vue; this is bumped after every stroke. */
const edits = ref(0)
const prop = computed(() => draft.value.props[selected.value])
/** The editor edits the one image in place; a new array only when the prop or its size changes. */
const frames = computed(() => prop.value ? [prop.value.pixels] : [])
const issues = computed(() => {
  void edits.value
  const out = decorProblems(draft.value)
  if (original && decorId(draft.value) === original) out.unshift('Give the copy a name of its own, so it does not replace the original.')
  return out
})
const saving = ref(false)
const saved = ref('')

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
  const current = prop.value!
  draft.value.props[selected.value] = resize(current, axis === 'width' ? value : current.width, axis === 'height' ? value : current.height)
  edits.value++
}
function togglePerch() {
  prop.value!.perch = prop.value!.perch ? null : { y: 0.1, from: 0.15, to: 0.85 }
}
function toggleAnimation(value: PropAnimation) {
  const list = prop.value!.animations
  const at = list.indexOf(value)
  if (at >= 0) list.splice(at, 1)
  else list.push(value)
}
/** Undo changed another prop's drawing: show that prop. */
function reveal(pixels: Uint8ClampedArray) {
  const index = draft.value.props.findIndex(p => p.pixels === pixels)
  if (index >= 0) selected.value = index
}

const thumb = thumbnailer()

async function save() {
  if (issues.value.length || saving.value) return
  saving.value = true
  saved.value = ''
  try {
    // As with pets: only a pack opened to edit, or saved before, may be
    // replaced; a new one (or a copy) gets an id of its own first.
    const replace = !!draft.value.id
    if (!replace) draft.value.id = uniqueId(decorId(draft.value))
    const id = await call<string>('save_decor_pack', { ...decorPayload(draft.value), inPlace: replace })
    if (id) {
      draft.value.id = id
      saved.value = draft.value.name.trim()
      emit('saved', id)
    }
  } finally { saving.value = false }
}
</script>

<template>
  <StudioShell v-model:name="draft.name" v-model:author="draft.author" class="decor-studio" name-label="Decor pack name" name-placeholder="Name your decor pack"
               author-placeholder="Made by (optional)" :issues="issues" :saved="!!saved">
    <template #actions>
      <button type="button" class="text-link" @click="emit('close')"><ArrowLeft :size="14" /> Back</button>
      <button type="button" class="primary-button" :disabled="!!issues.length || saving || !native" @click="save">{{ saving ? 'Saving…' : mode === 'in_place' ? 'Save changes' : 'Save decor' }}</button>
    </template>
    <template #saved>
      <span><strong>{{ saved }}</strong> is saved. Its props are in the tray when you decorate a window.</span>
    </template>

    <SpriteEditor :width="prop?.width ?? 32" :height="prop?.height ?? 32" :frames="frames" @edited="edits++" @reveal="reveal">
      <template #list>
        <aside class="clip-list" aria-label="Props">
          <h3>Props</h3>
          <button v-for="(p, i) in draft.props" :key="i" type="button" class="clip-row decor-prop" :class="{ selected: i === selected }" :aria-pressed="i === selected" @click="selected = i">
            <span class="clip-thumb" :class="{ empty: !thumb(p.pixels, p.width, p.height, edits) }">
              <img v-if="thumb(p.pixels, p.width, p.height, edits)" :src="thumb(p.pixels, p.width, p.height, edits)" alt="">
            </span>
            <span class="clip-text"><strong>{{ p.name }}</strong><small>{{ p.width }} × {{ p.height }}</small></span>
          </button>
          <button type="button" class="text-link decor-add" @click="addProp"><Plus :size="14" /> Add prop</button>
        </aside>
      </template>

      <template v-if="prop" #head>
        <div class="decor-head">
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
            <button type="button" class="text-link danger-link decor-remove" :disabled="draft.props.length < 2" @click="removeProp(selected)"><Trash2 :size="13" /> Remove prop</button>
          </div>
          <div v-if="prop.perch" class="decor-fields">
            <label>Ledge height <input v-model.number="prop.perch.y" type="range" min="0" max="1" step="0.01" aria-label="Ledge height"></label>
            <label>From <input v-model.number="prop.perch.from" type="range" min="0" max="1" step="0.01" aria-label="Ledge from" @input="edits++"></label>
            <label>To <input v-model.number="prop.perch.to" type="range" min="0" max="1" step="0.01" aria-label="Ledge to" @input="edits++"></label>
          </div>
          <div class="decor-fields" role="group" aria-label="Animations">
            <span class="decor-label">Animations</span>
            <label v-for="a in PROP_ANIMATIONS" :key="a.value" class="decor-check" :title="a.blurb">
              <input type="checkbox" :checked="prop.animations.includes(a.value)" :aria-label="a.label" @change="toggleAnimation(a.value)"> {{ a.label }}
            </label>
          </div>
          <p class="decor-note">The bottom row stands on the window's edge.<template v-if="prop.perch"> The dashed line is the ledge.</template></p>
        </div>
      </template>

      <template v-if="prop?.perch" #overlay="{ width, height }">
        <svg :viewBox="`0 0 ${width} ${height}`" preserveAspectRatio="none" aria-hidden="true">
          <line class="decor-ledge" :x1="prop.perch.from * width" :y1="prop.perch.y * height" :x2="prop.perch.to * width" :y2="prop.perch.y * height" />
        </svg>
      </template>
    </SpriteEditor>
  </StudioShell>
</template>

<style scoped>
.decor-head { display: grid; gap: 8px; margin-bottom: 4px; }
.decor-fields { display: flex; flex-wrap: wrap; gap: 6px 12px; align-items: center; }
.decor-fields label, .decor-label { display: flex; gap: 6px; align-items: center; font-size: 12.5px; color: var(--muted); }
.decor-fields input[type="number"] { width: 56px; }
.decor-remove { margin-left: auto; }
.decor-add { margin: 6px 8px 0; font-size: 12.5px; }
.decor-note { margin: 0 !important; color: var(--muted); font-size: 12px; }
.decor-ledge { stroke: #5b5bd6; stroke-width: 2; stroke-dasharray: 6 4; vector-effect: non-scaling-stroke; }
/* Hosted outside a fit-page layout, the stage still needs room to draw in. */
.decor-studio :deep(.canvas-stage) { min-height: 320px; }
</style>

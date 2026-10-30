<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Copy, Plus } from '@lucide/vue'
import { FRAME, MAX_FRAMES, clipDrawn, clipInfo, clipsFor, emptyClip, type Draft } from './model'
import SpriteEditor from './SpriteEditor.vue'
import { thumbnailer } from './thumbs'

/** The pet's Look tab: its animations, grouped by what they are for, around
 *  the shared pixel editor drawing the selected one. */
const props = defineProps<{ draft: Draft }>()
const emit = defineEmits<{ edited: [] }>()
/** Bumped with every edit so the clip list's thumbnails redraw. */
const version = ref(0)
function edited() { version.value++; emit('edited') }

/** One empty list for every undrawn clip, so the editor does not see a "new" clip on each render. */
const NO_FRAMES: Uint8ClampedArray[] = []
const clipName = ref('idle')
const clip = computed(() => props.draft.clips[clipName.value])
const groups = computed(() => {
  const list = clipsFor(props.draft.movement)
  return [
    { title: 'Basics', items: list.filter(c => c.group === 'basics') },
    { title: 'Getting around', items: list.filter(c => c.group === 'moves') },
    { title: 'Moods', items: list.filter(c => c.group === 'moods') },
    { title: 'Helping you', items: list.filter(c => c.group === 'assistant') },
  ].filter(g => g.items.length)
})
/** The drawn animations a new one can be copied from: Idle first, then Move. */
const copySources = computed(() => ['idle', 'move'].filter(n => n !== clipName.value && clipDrawn(props.draft.clips[n])).map(n => clipInfo(n)!))
/** What plays in place of an undrawn clip: its fallback if drawn, else Idle/Move, whichever exists. */
function fallbackFor(name: string) {
  const wanted = clipInfo(name)?.fallback ?? 'idle'
  if (clipDrawn(props.draft.clips[wanted])) return wanted
  const other = wanted === 'idle' ? 'move' : 'idle'
  return clipDrawn(props.draft.clips[other]) ? other : wanted
}
function pickClip(name: string) { clipName.value = name }
function startClip(copyFrom?: string) {
  const source = copyFrom ? props.draft.clips[copyFrom] : undefined
  props.draft.clips[clipName.value] = source
    ? { frames: source.frames.map(f => new Uint8ClampedArray(f)), fps: source.fps, loop: true }
    : emptyClip()
  edited()
}
function removeClip() {
  delete props.draft.clips[clipName.value]
  edited()
}
/** Undo changed a frame of another clip: show that clip. */
function reveal(pixels: Uint8ClampedArray) {
  const found = Object.entries(props.draft.clips).find(([, c]) => c.frames.includes(pixels))
  if (found) clipName.value = found[0]
}
// A movement change can hide the selected clip; fall back to idle.
watch(() => props.draft.movement, () => { if (!clipsFor(props.draft.movement).some(c => c.name === clipName.value)) pickClip('idle') })

const thumb = thumbnailer()
const firstFrame = (name: string) => thumb(props.draft.clips[name]?.frames[0], FRAME, FRAME, version.value)
</script>

<template>
  <SpriteEditor :width="FRAME" :height="FRAME" :frames="clip?.frames ?? NO_FRAMES" animation :max-frames="MAX_FRAMES"
                :fps="clip?.fps" :loop="clip?.loop" @update:fps="v => clip && (clip.fps = v)" @update:loop="v => clip && (clip.loop = v)"
                @edited="edited" @reveal="reveal">
    <template #list>
      <aside class="clip-list" aria-label="Animations">
        <template v-for="group in groups" :key="group.title">
          <h3>{{ group.title }}</h3>
          <button v-for="info in group.items" :key="info.name" type="button" class="clip-row" :class="{ selected: clipName === info.name }"
                  :aria-pressed="clipName === info.name" @click="pickClip(info.name)">
            <span class="clip-thumb" :class="{ empty: !clipDrawn(draft.clips[info.name]) }">
              <img v-if="firstFrame(info.name)" :src="firstFrame(info.name)" alt="">
            </span>
            <span class="clip-text">
              <strong>{{ info.label }}</strong>
              <small v-if="clipDrawn(draft.clips[info.name])">{{ draft.clips[info.name]!.frames.length }} frame{{ draft.clips[info.name]!.frames.length === 1 ? '' : 's' }}</small>
              <small v-else>Uses {{ clipInfo(fallbackFor(info.name))?.label }}</small>
            </span>
          </button>
        </template>
      </aside>
    </template>

    <template #head>
      <header class="canvas-head">
        <div>
          <h2>{{ clipInfo(clipName)?.label }}</h2>
          <p>{{ clipInfo(clipName)?.blurb }}</p>
        </div>
        <button v-if="clip" type="button" class="text-link danger-link" @click="removeClip">Remove animation</button>
      </header>
    </template>

    <template #empty>
      <div class="clip-start">
        <p>This animation is optional. Until you draw it, your pet plays <strong>{{ clipInfo(fallbackFor(clipName))?.label }}</strong> instead.</p>
        <div class="clip-start-actions">
          <button type="button" class="primary-button" @click="startClip()"><Plus :size="15" /> Start drawing</button>
          <button v-for="source in copySources" :key="source.name" type="button" @click="startClip(source.name)"><Copy :size="15" /> Start from {{ source.label }}</button>
        </div>
      </div>
    </template>
  </SpriteEditor>
</template>

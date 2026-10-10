<script setup lang="ts">
import { inject, ref } from 'vue'
import TermPane from './TermPane.vue'
import { ids, resize, type Layout } from './layout'
import { gridKey } from './gridContext'

defineOptions({ name: 'PaneNode' })
const props = defineProps<{ node: Layout }>()
const grid = inject(gridKey)!
const box = ref<HTMLDivElement | null>(null)
const dragging = ref(false)

const keyOf = (child: Layout) => child.type === 'pane' ? `p${child.id}` : `s${ids(child)[0]}`

function drag(index: number, event: PointerEvent) {
  const node = props.node
  if (node.type !== 'split' || !box.value) return
  const handle = event.currentTarget as HTMLElement
  handle.setPointerCapture(event.pointerId)
  const rect = box.value.getBoundingClientRect()
  const span = node.dir === 'row' ? rect.width : rect.height
  const start = node.dir === 'row' ? event.clientX : event.clientY
  const initial = [...node.sizes]
  dragging.value = true
  const move = (e: PointerEvent) => {
    const at = node.dir === 'row' ? e.clientX : e.clientY
    node.sizes = resize(initial, index, (at - start) / span)
  }
  const done = () => {
    dragging.value = false
    handle.removeEventListener('pointermove', move)
    handle.removeEventListener('pointerup', done)
    handle.removeEventListener('pointercancel', done)
    grid.changed()
  }
  handle.addEventListener('pointermove', move)
  handle.addEventListener('pointerup', done)
  handle.addEventListener('pointercancel', done)
}
</script>

<template>
  <TermPane v-if="node.type === 'pane'" :id="node.id" />
  <div v-else ref="box" class="split" :class="[`dir-${node.dir}`, { dragging }]">
    <template v-for="(child, i) in node.children" :key="keyOf(child)">
      <div class="split-cell" :style="{ flex: `${node.sizes[i]} 1 0` }"><PaneNode :node="child" /></div>
      <div v-if="i < node.children.length - 1" class="divider" :class="`dir-${node.dir}`" role="separator" :aria-orientation="node.dir === 'row' ? 'vertical' : 'horizontal'" @pointerdown.prevent="drag(i, $event)" />
    </template>
  </div>
</template>

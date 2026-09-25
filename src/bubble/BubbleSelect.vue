<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'

/** A listbox drawn inside the bubble. A native `<select>` opens its menu as a
 *  separate window on some platforms, which takes focus from the bubble and
 *  would dismiss it mid-choice. */
const props = defineProps<{ options: string[]; modelValue: number; label: string }>()
const emit = defineEmits<{ 'update:modelValue': [index: number] }>()
const open = ref(false)
const active = ref(0)
const upward = ref(false)
const root = ref<HTMLElement>()
const list = ref<HTMLElement>()
const id = `bubble-select-${Math.random().toString(36).slice(2, 8)}`
const current = computed(() => props.options[props.modelValue] ?? '')

function outside(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) close()
}
function close() {
  open.value = false
  document.removeEventListener('pointerdown', outside, true)
}
async function show() {
  active.value = Math.max(0, props.modelValue)
  // Open towards whichever side has room: the window is only as tall as the
  // bubble, and a list past its edge would be cut off.
  const rect = root.value?.getBoundingClientRect()
  upward.value = !!rect && rect.bottom + 170 > window.innerHeight && rect.top > window.innerHeight - rect.bottom
  open.value = true
  document.addEventListener('pointerdown', outside, true)
  await nextTick()
  list.value?.querySelector<HTMLElement>('[aria-selected="true"]')?.scrollIntoView?.({ block: 'nearest' })
}
function choose(index: number) {
  emit('update:modelValue', index)
  close()
}
function key(event: KeyboardEvent) {
  const last = props.options.length - 1
  if (!open.value) {
    if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) { event.preventDefault(); void show() }
    return
  }
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close() }
  else if (event.key === 'ArrowDown') { event.preventDefault(); active.value = Math.min(last, active.value + 1) }
  else if (event.key === 'ArrowUp') { event.preventDefault(); active.value = Math.max(0, active.value - 1) }
  else if (event.key === 'Home') { event.preventDefault(); active.value = 0 }
  else if (event.key === 'End') { event.preventDefault(); active.value = last }
  else if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); choose(active.value) }
  else if (event.key === 'Tab') close()
}
watch(active, async () => {
  await nextTick()
  list.value?.querySelector<HTMLElement>('.active')?.scrollIntoView?.({ block: 'nearest' })
})
onUnmounted(close)
defineExpose({ close })
</script>

<template>
  <div ref="root" class="bubble-select" :class="{ open, upward }">
    <button
      type="button"
      class="bubble-select-button"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-controls="id"
      :aria-label="`${label}: ${current}`"
      :aria-activedescendant="open ? `${id}-${active}` : undefined"
      @click="open ? close() : show()"
      @keydown="key"
    >
      <span>{{ current }}</span>
      <svg class="chevron" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="m3 4.5 3 3 3-3" /></svg>
    </button>
    <ul v-if="open" :id="id" ref="list" role="listbox" :aria-label="label" class="bubble-select-list">
      <li
        v-for="(option, index) in options"
        :id="`${id}-${index}`"
        :key="`${index}-${option}`"
        role="option"
        :aria-selected="index === modelValue"
        :class="{ active: index === active }"
        @pointerenter="active = index"
        @click="choose(index)"
      >{{ option }}</li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'

/** The app's dropdown: a listbox drawn in the page. A native `<select>` opens its
 *  menu as a separate window on some platforms, which takes focus from the pet
 *  bubble and would dismiss it mid-choice. */
const props = defineProps<{ options: string[]; modelValue: number; label: string; placeholder?: string; disabled?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [index: number] }>()
const open = ref(false)
const active = ref(0)
const upward = ref(false)
const root = ref<HTMLElement>()
const list = ref<HTMLElement>()
const id = `dropdown-${Math.random().toString(36).slice(2, 8)}`
const current = computed(() => props.options[props.modelValue] ?? props.placeholder ?? '')

function outside(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) close()
}
function close() {
  open.value = false
  document.removeEventListener('pointerdown', outside, true)
}
async function show() {
  if (props.disabled) return
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
  <div ref="root" class="dropdown" :class="{ open, upward }">
    <button
      type="button"
      class="dropdown-button"
      :disabled="disabled"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-controls="id"
      :aria-label="`${label}: ${current}`"
      :aria-activedescendant="open ? `${id}-${active}` : undefined"
      @click="open ? close() : show()"
      @keydown="key"
    >
      <span :class="{ placeholder: modelValue < 0 }">{{ current }}</span>
      <svg class="chevron" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="m3 4.5 3 3 3-3" /></svg>
    </button>
    <ul v-if="open" :id="id" ref="list" role="listbox" :aria-label="label" class="dropdown-list">
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

<style>
.dropdown { position: relative; z-index: 3; min-width: 0; }
.dropdown.open { z-index: 40; }
.dropdown .dropdown-button { display: flex; align-items: center; justify-content: space-between; gap: 8px; width: 100%; height: 32px; min-height: 0; padding: 0 12px 0 14px; border-radius: 12px; border: 1px solid var(--line); background: var(--soft); color: var(--ink); font: inherit; font-size: 12.5px; font-weight: 500; text-align: left; cursor: pointer; }
.dropdown .dropdown-button:not(:disabled):hover { border-color: var(--line-strong); background: var(--soft); }
.dropdown.open .dropdown-button { border-color: color-mix(in srgb, var(--accent) 45%, var(--line)); }
.dropdown-button span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dropdown-button .placeholder { color: var(--muted); }
.dropdown .chevron { flex-shrink: 0; fill: none; stroke: var(--muted); stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; transition: transform 220ms; }
.dropdown.open .chevron { transform: rotate(180deg); }
.dropdown-list {
  position: absolute; left: 0; right: 0; top: calc(100% + 4px); margin: 0; padding: 3px; list-style: none; max-height: 184px; overflow-y: auto;
  background: var(--surface); color: var(--ink); border: 1px solid var(--line-strong); border-radius: 12px; box-shadow: 0 6px 18px rgb(0 0 0 / .14); animation: dropdown-fade 160ms ease-out;
}
.dropdown.upward .dropdown-list { top: auto; bottom: calc(100% + 4px); }
.dropdown-list li { padding: 6px 10px; border-radius: 9px; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; }
.dropdown-list li.active { background: var(--soft); }
.dropdown-list li[aria-selected=true] { color: var(--accent); font-weight: 500; }
@keyframes dropdown-fade { from { opacity: 0; } }
@media (prefers-reduced-motion: reduce) {
  .dropdown .chevron { transition: none; }
  .dropdown-list { animation: none; }
}
</style>

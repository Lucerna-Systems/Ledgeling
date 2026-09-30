<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { terms } from '../terminals'

/** Shows one live terminal. The terminal itself belongs to `terminals.ts`;
 *  this only borrows its element while visible, so whichever view became
 *  visible last holds it. `focus` gives it the keyboard as well. */
const props = withDefaults(defineProps<{ id: number; active?: boolean; focus?: boolean }>(), { active: true, focus: true })
const host = ref<HTMLDivElement | null>(null)
let observer: ResizeObserver | null = null

function fit() {
  const entry = terms.get(props.id)
  if (!entry || !host.value || host.value.clientWidth < 20 || entry.element.parentElement !== host.value) return
  try { entry.fit.fit() } catch { /* hidden */ }
}

function mount() {
  const entry = terms.get(props.id)
  if (!entry || !host.value) return
  if (entry.element.parentElement !== host.value) host.value.appendChild(entry.element)
  if (!entry.opened) { entry.term.open(entry.element); entry.opened = true }
  requestAnimationFrame(() => { fit(); if (props.active && props.focus) entry.term.focus() })
}

onMounted(() => {
  if (props.active) mount()
  observer = new ResizeObserver(() => fit())
  if (host.value) observer.observe(host.value)
})
watch(() => props.id, () => { if (props.active) mount() })
watch(() => props.active, active => { if (active) mount() })
watch(() => props.focus, focus => { if (focus && props.active) terms.get(props.id)?.term.focus() })
onBeforeUnmount(() => {
  observer?.disconnect()
  const entry = terms.get(props.id)
  if (entry && entry.element.parentElement === host.value) entry.element.remove()
})
</script>

<template><div ref="host" class="term-view" /></template>

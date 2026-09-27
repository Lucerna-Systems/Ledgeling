<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { terms } from '../terminals'

/** Shows one live terminal. The terminal itself belongs to `terminals.ts`;
 *  this only borrows its element while visible. */
const props = defineProps<{ id: number; active?: boolean }>()
const host = ref<HTMLDivElement | null>(null)
let observer: ResizeObserver | null = null

function fit() {
  const entry = terms.get(props.id)
  if (!entry || !host.value || host.value.clientWidth < 20) return
  try { entry.fit.fit() } catch { /* hidden */ }
}

function mount() {
  const entry = terms.get(props.id)
  if (!entry || !host.value) return
  if (entry.element.parentElement !== host.value) host.value.appendChild(entry.element)
  if (!entry.opened) { entry.term.open(entry.element); entry.opened = true }
  requestAnimationFrame(() => { fit(); if (props.active !== false) entry.term.focus() })
}

onMounted(() => {
  mount()
  observer = new ResizeObserver(() => fit())
  if (host.value) observer.observe(host.value)
})
watch(() => props.id, mount)
watch(() => props.active, active => { if (active) mount() })
onBeforeUnmount(() => {
  observer?.disconnect()
  const entry = terms.get(props.id)
  if (entry && entry.element.parentElement === host.value) entry.element.remove()
})
</script>

<template><div ref="host" class="term-view" /></template>

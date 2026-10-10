<script setup lang="ts">
import { nextTick, ref } from 'vue'

/** A tab's name, renamed by double-clicking it. Enter keeps the new name, Escape or clicking away keeps the old. */
const props = defineProps<{ title: string }>()
const emit = defineEmits<{ rename: [title: string] }>()
const editing = ref(false)
const text = ref('')
const input = ref<HTMLInputElement | null>(null)

async function begin() {
  text.value = props.title
  editing.value = true
  await nextTick()
  input.value?.focus()
  input.value?.select()
}
function commit() {
  if (!editing.value) return
  editing.value = false
  emit('rename', text.value)
}
function cancel() { editing.value = false }
</script>

<template>
  <input v-if="editing" ref="input" v-model="text" class="rename-input" maxlength="40" aria-label="Terminal name"
         @click.stop @pointerdown.stop @dblclick.stop @keydown.enter.stop.prevent="commit" @keydown.esc.stop.prevent="cancel" @keydown.stop @blur="commit">
  <span v-else class="rename-title" title="Double-click to rename" @dblclick.stop="begin">{{ title }}</span>
</template>

<style scoped>
.rename-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
.rename-input { width: 110px; min-width: 0; height: 20px; padding: 0 4px; border: 1px solid var(--accent); border-radius: 5px; background: var(--surface); color: var(--ink); font: inherit; outline: none; }
</style>

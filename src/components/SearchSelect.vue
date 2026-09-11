<script setup lang="ts">
import { computed, ref, watch } from 'vue'
const props = withDefaults(defineProps<{ modelValue: string; options: string[]; placeholder?: string; disabled?: boolean }>(), { placeholder: '', disabled: false })
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
const open = ref(false)
const query = ref(props.modelValue)
const active = ref(-1)
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  return q ? props.options.filter(o => o.toLowerCase().includes(q)) : props.options
})
// External changes (a saved model, a bubble pick) sync when closed; while the
// user is typing, the query belongs to them.
watch(() => props.modelValue, value => { if (!open.value) query.value = value })
function focusInput() { open.value = true; active.value = -1 }
function commit(value: string) { emit('update:modelValue', value); query.value = value; open.value = false }
function keydown(event: KeyboardEvent) {
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    open.value = true
    if (!filtered.value.length) return
    const delta = event.key === 'ArrowDown' ? 1 : -1
    active.value = (active.value + delta + filtered.value.length) % filtered.value.length
  } else if (event.key === 'Enter') {
    // Free-text commit: custom endpoints expose model ids their /models
    // listing misses, so an unmatched typed value is still a valid pick.
    event.preventDefault()
    commit(active.value >= 0 ? filtered.value[active.value] : query.value)
  } else if (event.key === 'Escape') {
    open.value = false
  }
}
// Blurring never commits — typed text only counts when deliberately chosen.
function blur() { open.value = false; query.value = props.modelValue }
</script>
<template>
  <div class="search-select">
    <input v-model="query" type="text" role="combobox" :aria-expanded="open" aria-autocomplete="list"
      :placeholder="placeholder" :disabled="disabled" spellcheck="false"
      @focus="focusInput" @keydown="keydown" @blur="blur">
    <ul v-if="open && filtered.length" class="search-select-list" role="listbox">
      <li v-for="(option, index) in filtered" :key="option" role="option" :aria-selected="active === index"
        :class="{ active: active === index }" @mousedown.prevent @click="commit(option)" @mousemove="active = index">{{ option }}</li>
    </ul>
  </div>
</template>
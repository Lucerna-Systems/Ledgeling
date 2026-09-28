<script setup lang="ts">
import { computed, ref } from 'vue'
import { Check, Search } from '@lucide/vue'
import Avatar from './Avatar.vue'
import { labelStyle } from './format'

/** Choose labels or people; changes apply together when the picker closes. */
const props = defineProps<{ title: string; options: { value: string; color?: string; hint?: string }[]; selected: string[]; people?: boolean }>()
const emit = defineEmits<{ apply: [add: string[], remove: string[]]; close: [] }>()

const chosen = ref(new Set(props.selected))
const query = ref('')
const shown = computed(() => {
  const q = query.value.trim().toLowerCase()
  return props.options.filter(o => !q || o.value.toLowerCase().includes(q))
})
function toggle(value: string) {
  const next = new Set(chosen.value)
  if (next.has(value)) next.delete(value)
  else next.add(value)
  chosen.value = next
}
function done() {
  const before = new Set(props.selected)
  const add = [...chosen.value].filter(v => !before.has(v))
  const remove = [...before].filter(v => !chosen.value.has(v))
  if (add.length || remove.length) emit('apply', add, remove)
  emit('close')
}
</script>

<template>
  <div class="popover-scrim" @click="done" />
  <div class="gh-picker" role="dialog" :aria-label="title" @keydown.esc="done">
    <div class="gh-picker-head">{{ title }}</div>
    <label class="gh-search">
      <Search :size="12" class="muted" />
      <input v-model="query" v-focus :placeholder="`Filter ${title.toLowerCase()}`" spellcheck="false" @keydown.enter.prevent="shown[0] && toggle(shown[0].value)">
    </label>
    <div class="gh-picker-list">
      <button v-for="o in shown" :key="o.value" class="bare gh-picker-row" :aria-pressed="chosen.has(o.value)" @click="toggle(o.value)">
        <span class="gh-tick"><Check v-if="chosen.has(o.value)" :size="12" /></span>
        <Avatar v-if="people" :login="o.value" :size="18" />
        <span v-else class="gh-swatch" :style="labelStyle(o.color ?? '')" />
        <span class="grow">
          <strong>{{ o.value }}</strong>
          <small v-if="o.hint" class="muted">{{ o.hint }}</small>
        </span>
      </button>
      <p v-if="!shown.length" class="muted small gh-pad">Nothing matches.</p>
    </div>
    <div class="gh-picker-foot"><button class="primary-button" @click="done">Done</button></div>
  </div>
</template>

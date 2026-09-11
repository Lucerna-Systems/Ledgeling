<script setup lang="ts">
import { Search, X } from '@lucide/vue'
defineProps<{ label: string; count: number; options?: { value: string; label: string }[] }>()
const query = defineModel<string>('query', { default: '' })
const filter = defineModel<string>('filter', { default: 'all' })
</script>
<template>
  <div class="collection-toolbar">
    <label class="search-field">
      <Search :size="16" />
      <input v-model="query" type="search" :aria-label="label" :placeholder="label">
    </label>
    <select v-if="options" v-model="filter" :aria-label="label + ' filter'">
      <option v-for="option in options" :key="option.value" :value="option.value">{{ option.label }}</option>
    </select>
    <button v-if="query || filter !== 'all'" class="icon-button" aria-label="Clear search and filters" @click="query = ''; filter = 'all'"><X :size="16" /></button>
    <span class="result-count" role="status">{{ count }} {{ count === 1 ? 'result' : 'results' }}</span>
  </div>
</template>

<script setup lang="ts">
/** A compact dropdown filter; the first option is the "no filter" default. */
const props = defineProps<{ label: string; options: { value: string; label: string }[] }>()
const model = defineModel<string>({ required: true })
</script>
<template>
  <label class="filter-pill" :class="{ active: model !== props.options[0]?.value }">
    <span class="sr-only">{{ label }}</span>
    <select v-model="model" :aria-label="label">
      <option v-for="option in options" :key="option.value" :value="option.value">{{ option.label }}</option>
    </select>
  </label>
</template>

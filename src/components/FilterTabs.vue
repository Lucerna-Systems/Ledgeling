<script setup lang="ts">
defineProps<{ label: string; options: { value: string; label: string; count?: number }[] }>()
const model = defineModel<string>({ required: true })
</script>
<template>
  <div class="filter-tabs" role="radiogroup" :aria-label="label">
    <button v-for="option in options" :key="option.value" type="button" role="radio" :aria-checked="model === option.value" @click="model = option.value">
      {{ option.label }}<span v-if="option.count !== undefined" class="filter-count">{{ option.count }}</span>
    </button>
  </div>
</template>

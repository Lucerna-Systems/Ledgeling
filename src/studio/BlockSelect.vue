<script setup lang="ts">
import { computed } from 'vue'
import Dropdown from '../components/Dropdown.vue'

/** The shared dropdown, keyed by value, sized to sit inside a sentence. */
const props = defineProps<{ options: readonly { value: string; label: string }[]; label: string }>()
const model = defineModel<string>({ required: true })
const index = computed(() => props.options.findIndex(o => o.value === model.value))
</script>
<template>
  <Dropdown class="block-select" :label="label" :options="options.map(o => o.label)" :model-value="index"
            placeholder="choose…" @update:model-value="model = options[$event]!.value" />
</template>

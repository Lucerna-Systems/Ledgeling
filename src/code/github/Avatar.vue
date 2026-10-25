<script setup lang="ts">
import { computed } from 'vue'
import { hue, initials } from './format'

/** A drawn avatar: GitHub pictures are never fetched. */
const props = defineProps<{ login: string | null | undefined; size?: number }>()
const name = computed(() => props.login ?? 'ghost')
</script>

<template>
  <span class="gh-avatar" :title="name" :style="{ '--h': hue(name), width: `${size ?? 20}px`, height: `${size ?? 20}px`, fontSize: `${Math.round((size ?? 20) * 0.42)}px` }">{{ initials(name) }}</span>
</template>

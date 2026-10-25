<script setup lang="ts">
import { computed } from 'vue'
import { Bird } from '@lucide/vue'
import { packs, pets } from '../state'
import type { TaskPet } from './types'

/** `petId` is the fallback for tasks from before pets were stamped on them:
 *  ids are handed out in launch order, so the live pet with it is the best guess. */
const props = withDefaults(defineProps<{ pet: TaskPet | null; petId?: number | null; size?: number }>(), { petId: null, size: 22 })
const packId = computed(() => props.pet?.packId ?? pets.value.find(p => p.id === props.petId)?.packId ?? null)
const name = computed(() => props.pet?.name ?? pets.value.find(p => p.id === props.petId)?.name)
const preview = computed(() => packs.value.find(p => p.summary.id === packId.value)?.summary.preview ?? null)
</script>

<template>
  <span class="pet-face" :style="{ width: `${size}px`, height: `${size}px` }" :title="name">
    <img v-if="preview" :src="preview" alt="">
    <Bird v-else :size="Math.round(size * 0.62)" :stroke-width="1.8" />
  </span>
</template>

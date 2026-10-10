<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import { question } from '../confirm'

const yes = ref<HTMLButtonElement | null>(null)
// Enter answers yes from the focused button; Escape (on the dialog) answers no.
watch(question, q => { if (q) void nextTick(() => yes.value?.focus()) })
</script>

<template>
  <template v-if="question">
    <div class="dialog-scrim" @pointerdown="question.answer(false)" />
    <div class="dialog" role="alertdialog" aria-modal="true" :aria-label="question.action" @keydown.esc.stop="question.answer(false)">
      <p>{{ question.message }}</p>
      <div class="card-actions">
        <button ref="yes" :class="question.danger ? 'danger-solid' : 'primary-button'" @click="question.answer(true)">{{ question.action }}</button>
        <button class="ghost" @click="question.answer(false)">Cancel</button>
      </div>
    </div>
  </template>
</template>

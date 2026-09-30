<script setup lang="ts">
import TerminalGrid from '../components/TerminalGrid.vue'

defineProps<{ active?: boolean }>()
</script>

<template>
  <section>
    <TerminalGrid :active="!!active" />
  </section>
</template>

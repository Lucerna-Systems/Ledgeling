<script setup lang="ts">
import { defineAsyncComponent } from 'vue'
// One webview loads exactly one of these, so each is its own chunk: the pet overlay never parses the dashboard, and vice versa.
const PetWindow = defineAsyncComponent(() => import('./pet/PetWindow.vue'))
const PetBubble = defineAsyncComponent(() => import('./bubble/PetBubble.vue'))
const Companion = defineAsyncComponent(() => import('./Companion.vue'))
const query = new URLSearchParams(location.search)
const pet = query.has('pet')
const bubble = query.has('bubble')
</script>
<template><PetWindow v-if="pet" /><PetBubble v-else-if="bubble" /><Companion v-else /></template>

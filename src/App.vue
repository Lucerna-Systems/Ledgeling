<script setup lang="ts">
import PetWindow from './pet/PetWindow.vue'
import PetBubble from './bubble/PetBubble.vue'
import Companion from './Companion.vue'
const query = new URLSearchParams(location.search)
const pet = query.has('pet')
const bubble = query.has('bubble')
</script>
<template><PetWindow v-if="pet" /><PetBubble v-else-if="bubble" /><Companion v-else /></template>

<script setup lang="ts">
import PetWindow from './pet/PetWindow.vue'
import Companion from './Companion.vue'
const pet = new URLSearchParams(location.search).has('pet')
</script>
<template><PetWindow v-if="pet" /><Companion v-else /></template>

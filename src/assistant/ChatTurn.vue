<script setup lang="ts">
import { computed } from 'vue'
import { FolderPlus, Paperclip, Sparkles } from '@lucide/vue'
import PetFace from './PetFace.vue'
import TaskCard from './TaskCard.vue'
import { presets, type Task, type TaskPet } from './types'

const props = defineProps<{ task: Task; pet: TaskPet | null }>()
const preset = computed(() => presets.find(p => p.value === props.task.request.preset)?.label)
const short = (path: string) => path.split(/[\\/]/).filter(Boolean).at(-1) ?? path
</script>

<template>
  <div class="chat-turn">
    <div class="chat-bubble you">
      <p v-if="preset" class="chat-preset"><Sparkles :size="12" /> {{ preset }}</p>
      <p class="chat-prompt">{{ task.request.prompt }}</p>
      <ul v-if="task.request.roots.length || task.request.attachments.length" class="attachment-list">
        <li v-for="path in task.request.roots" :key="path" :title="path"><FolderPlus :size="12" /> {{ short(path) }}</li>
        <li v-for="path in task.request.attachments" :key="path" :title="path"><Paperclip :size="12" /> {{ short(path) }}</li>
      </ul>
    </div>
    <div class="chat-reply">
      <PetFace :pet="task.pet ?? pet" :pet-id="task.request.petId" :size="30" />
      <TaskCard :task="task" in-chat />
    </div>
  </div>
</template>

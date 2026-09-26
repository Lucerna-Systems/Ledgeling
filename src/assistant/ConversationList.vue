<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { Pencil, Plus } from '@lucide/vue'
import PetFace from './PetFace.vue'
import { activeConversation, conversations, renameConversation } from './state'

/** `current` is whether the Assistant page is the one showing. */
defineProps<{ current: boolean }>()
const emit = defineEmits<{ open: [] }>()

const editing = ref<number | null>(null)
const draft = ref('')
const field = ref<HTMLInputElement[]>([])

function open(id: number | null) {
  activeConversation.value = id
  emit('open')
}
async function startRename(id: number, title: string) {
  editing.value = id
  draft.value = title
  await nextTick()
  field.value[0]?.select()
}
async function commit(id: number) {
  // Enter and Escape both remove the field, which then blurs; only the first counts.
  if (editing.value !== id) return
  editing.value = null
  const title = draft.value.trim()
  if (title !== conversations.value.find(chat => chat.id === id)?.title) {
    try { await renameConversation(id, title) } catch { /* shared error banner */ }
  }
}
</script>

<template>
  <ul class="chat-list" aria-label="Conversations">
    <li>
      <button type="button" class="chat-item new-chat" :class="{ selected: current && activeConversation === null }" @click="open(null)">
        <Plus :size="15" /> New chat
      </button>
    </li>
    <li v-for="chat in conversations" :key="chat.id" class="chat-row">
      <label v-if="editing === chat.id" class="chat-item editing">
        <PetFace :pet="chat.pet" :pet-id="chat.petId" :size="22" />
        <input ref="field" v-model="draft" maxlength="80" aria-label="Conversation name"
               @keydown.enter.prevent="commit(chat.id)" @keydown.esc.prevent="editing = null" @blur="commit(chat.id)">
      </label>
      <template v-else>
        <button type="button" class="chat-item" :class="{ selected: current && activeConversation === chat.id }"
                :aria-current="current && activeConversation === chat.id ? 'page' : undefined" :title="chat.title"
                @click="open(chat.id)" @dblclick="startRename(chat.id, chat.title)">
          <PetFace :pet="chat.pet" :pet-id="chat.petId" :size="22" />
          <span class="chat-title">{{ chat.title }}</span>
          <span v-if="chat.waiting" class="chat-waiting" aria-label="Needs you" />
        </button>
        <button type="button" class="chat-rename" :aria-label="`Rename ${chat.title}`" @click="startRename(chat.id, chat.title)">
          <Pencil :size="12" />
        </button>
      </template>
    </li>
  </ul>
</template>

<script setup lang="ts">
import { nextTick, reactive, ref } from 'vue'
import { ChevronDown, Pencil, Plus } from '@lucide/vue'
import PetFace from './PetFace.vue'
import { activeConversation, assignedPet, conversations, conversationsByPet, livePet, renameConversation, type PetChats } from './state'

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
/** A new chat already pointed at this pet, if it is out on the desktop. */
function newWith(group: PetChats) {
  open(null)
  assignedPet.value = group.key === 'any' ? null : livePet(group.petId, group.pet)
}
const collapsed = reactive(new Set<string>())
function toggle(key: string) {
  if (collapsed.has(key)) collapsed.delete(key)
  else collapsed.add(key)
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
    <li v-for="group in conversationsByPet" :key="group.key" class="pet-chats">
      <div class="pet-chats-head">
        <button type="button" class="chat-item pet-chats-toggle" :aria-expanded="!collapsed.has(group.key)" @click="toggle(group.key)">
          <PetFace :pet="group.pet" :pet-id="group.petId" :size="22" />
          <span class="chat-title">{{ group.name }}</span>
          <span v-if="group.waiting && collapsed.has(group.key)" class="chat-waiting" aria-label="Needs you" />
          <span class="pet-chats-count">{{ group.chats.length }}</span>
          <ChevronDown :size="13" class="pet-chats-chevron" />
        </button>
        <button type="button" class="chat-rename" :aria-label="`New chat with ${group.name}`" :title="`New chat with ${group.name}`" @click="newWith(group)">
          <Plus :size="13" />
        </button>
      </div>
      <ul v-if="!collapsed.has(group.key)" class="pet-chats-list" :aria-label="`Chats with ${group.name}`">
        <li v-for="chat in group.chats" :key="chat.id" class="chat-row">
          <label v-if="editing === chat.id" class="chat-item editing">
            <input ref="field" v-model="draft" maxlength="80" aria-label="Conversation name"
                   @keydown.enter.prevent="commit(chat.id)" @keydown.esc.prevent="editing = null" @blur="commit(chat.id)">
          </label>
          <template v-else>
            <button type="button" class="chat-item" :class="{ selected: current && activeConversation === chat.id }"
                    :aria-current="current && activeConversation === chat.id ? 'page' : undefined" :title="chat.title"
                    @click="open(chat.id)" @dblclick="startRename(chat.id, chat.title)">
              <span class="chat-title">{{ chat.title }}</span>
              <span v-if="chat.waiting" class="chat-waiting" aria-label="Needs you" />
            </button>
            <button type="button" class="chat-rename" :aria-label="`Rename ${chat.title}`" @click="startRename(chat.id, chat.title)">
              <Pencil :size="12" />
            </button>
          </template>
        </li>
      </ul>
    </li>
  </ul>
</template>

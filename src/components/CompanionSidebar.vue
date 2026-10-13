<script setup lang="ts">
import { computed } from 'vue'
import { Bird, Bot, ChevronLeft, ChevronRight, Code2, Eye, EyeOff, History, MessageCircle, PawPrint, Pause, Play, Settings, ShieldCheck } from '@lucide/vue'
import ConversationList from '../assistant/ConversationList.vue'
import { SIDEBAR_MAX, SIDEBAR_MIN, useSidebar } from '../sidebar'
import type { Page } from '../types'

const props = defineProps<{
  page: Page
  section: Page
  waiting: number
  status: string
  quiet: boolean
  offline: boolean
  petsCount: number
  developerMode?: boolean
  paused?: boolean
  hidden?: boolean
  hasConversations: boolean
}>()

const emit = defineEmits<{
  (e: 'update:page', page: Page): void
  (e: 'action', actionName: string): void
  (e: 'openCode'): void
}>()

const nav = [
  { page: 'assistant', label: 'Assistant', icon: MessageCircle },
  { page: 'providers', label: 'AI providers', icon: Bot },
  { page: 'pets', label: 'Pets', icon: PawPrint },
  { page: 'history', label: 'History', icon: History },
  { page: 'settings', label: 'Settings', icon: Settings },
] as const

const sidebar = useSidebar()
const { width: sidebarWidth, collapsed, resizing } = sidebar

function setPage(next: Page) {
  emit('update:page', next)
}
</script>

<template>
  <div class="sidebar-shell" :class="{ collapsed, resizing }" :style="{ width: collapsed ? undefined : `${sidebarWidth}px` }">
    <aside class="sidebar">
      <a class="brand" href="#" :title="collapsed ? 'Ledgeling' : undefined" @click.prevent="setPage('assistant')">
        <span class="brand-mark"><Bird :size="22" :stroke-width="1.7" /></span>
        <span class="label">ledgeling</span>
      </a>

      <nav aria-label="Main navigation">
        <button
          v-for="item in nav"
          :key="item.page"
          :class="{ selected: section === item.page }"
          :title="collapsed ? item.label : undefined"
          :aria-current="section === item.page ? 'page' : undefined"
          @click="setPage(item.page)"
        >
          <component :is="item.icon" :size="18" :stroke-width="1.8" />
          <span class="label">{{ item.label }}</span>
          <span v-if="item.page === 'history' && waiting" class="nav-badge" :aria-label="`${waiting} waiting for you`">{{ waiting }}</span>
        </button>

        <button
          v-if="developerMode"
          class="code-entry"
          :title="collapsed ? 'Coding mode' : 'Open the coding window beside your browser'"
          :disabled="offline"
          @click="emit('openCode')"
        >
          <Code2 :size="18" :stroke-width="1.8" />
          <span class="label">Coding mode</span>
        </button>
      </nav>

      <section v-if="hasConversations && !collapsed" class="chat-section" aria-labelledby="chats-heading">
        <h2 id="chats-heading" class="chat-section-title">Chats</h2>
        <ConversationList :current="page === 'assistant'" @open="setPage('assistant')" />
      </section>

      <div class="sidebar-bottom">
        <span class="sidebar-status" :title="collapsed ? status : undefined">
          <span class="live-dot" :class="{ sleeping: quiet }" />
          <span class="label">{{ status }}</span>
        </span>

        <div v-if="petsCount || paused || hidden" class="sidebar-controls">
          <button
            :class="{ active: paused }"
            :disabled="offline"
            :aria-label="paused ? 'Resume' : 'Pause'"
            :title="collapsed ? (paused ? 'Resume' : 'Pause') : undefined"
            @click="emit('action', 'pause')"
          >
            <Play v-if="paused" :size="14" /><Pause v-else :size="14" />
            <span class="label">{{ paused ? 'Resume' : 'Pause' }}</span>
          </button>
          <button
            :class="{ active: hidden }"
            :disabled="offline"
            :aria-label="hidden ? 'Show' : 'Hide'"
            :title="collapsed ? (hidden ? 'Show' : 'Hide') : undefined"
            @click="emit('action', 'visibility')"
          >
            <Eye v-if="hidden" :size="14" /><EyeOff v-else :size="14" />
            <span class="label">{{ hidden ? 'Show' : 'Hide' }}</span>
          </button>
        </div>

        <span class="sidebar-private">
          <ShieldCheck :size="13" />
          <span class="label">Stays on this computer</span>
        </span>
      </div>
    </aside>

    <div
      v-if="!collapsed"
      class="sidebar-resize"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize sidebar"
      tabindex="0"
      :aria-valuenow="sidebarWidth"
      :aria-valuemin="SIDEBAR_MIN"
      :aria-valuemax="SIDEBAR_MAX"
      @pointerdown.prevent="sidebar.startResize"
      @dblclick="sidebar.reset"
      @keydown.left.prevent="sidebar.nudge(-16)"
      @keydown.right.prevent="sidebar.nudge(16)"
    />

    <button
      type="button"
      class="sidebar-toggle"
      :aria-label="collapsed ? 'Expand sidebar' : 'Collapse sidebar'"
      :aria-expanded="!collapsed"
      @click="collapsed = !collapsed"
    >
      <ChevronRight v-if="collapsed" :size="14" /><ChevronLeft v-else :size="14" />
    </button>
  </div>
</template>

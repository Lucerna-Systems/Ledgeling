<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import AssistantPanel from './assistant/AssistantPanel.vue'
import HistoryPanel from './assistant/HistoryPanel.vue'
import ProvidersPanel from './assistant/ProvidersPanel.vue'
import { activeConversation, conversationOf, conversations, startTasks, assignedPet, droppedFiles, tasks } from './assistant/state'
import ConversationList from './assistant/ConversationList.vue'
import type { Task } from './assistant/types'
import { ArrowLeft, Bird, Bot, ChevronLeft, ChevronRight, Eye, EyeOff, History, MessageCircle, PawPrint, Pause, Play, Settings, ShieldCheck, Monitor, X } from '@lucide/vue'
import { action, busy, error, native, notes, pets, settings, snapshot, start } from './state'
import PacksPanel from './panels/PacksPanel.vue'
import SettingsPanel from './panels/SettingsPanel.vue'
import LabPanel from './panels/LabPanel.vue'
import AwarenessPanel from './panels/AwarenessPanel.vue'
import DesktopPanel from './panels/DesktopPanel.vue'
import PetStudio from './studio/PetStudio.vue'
import { syncAppIcon } from './appIcon'
import { SIDEBAR_MAX, SIDEBAR_MIN, useSidebar } from './sidebar'

type Page = 'pets' | 'assistant' | 'providers' | 'history' | 'settings' | 'desktop' | 'awareness' | 'lab' | 'studio'
const nav = [
  { page: 'assistant', label: 'Assistant', icon: MessageCircle },
  { page: 'providers', label: 'AI providers', icon: Bot },
  { page: 'pets', label: 'Pets', icon: PawPrint },
  { page: 'history', label: 'History', icon: History },
  { page: 'settings', label: 'Settings', icon: Settings },
] as const
/** Pages reached from inside another section, so the sidebar stays short. */
const subpages: Partial<Record<Page, Page>> = { desktop: 'settings', awareness: 'settings', lab: 'settings', studio: 'pets' }
const titles: Partial<Record<Page, string>> = { pets: 'Pets', settings: 'Settings', studio: 'Pet Studio' }

const page = ref<Page>('assistant')
const sidebar = useSidebar()
const { width: sidebarWidth, collapsed, resizing } = sidebar
/** Where a sub-page's back button returns to: the page that opened it. */
const back = ref<Page>('settings')
const section = computed(() => subpages[page.value] ?? page.value)
function go(next: Page) {
  if (subpages[next]) back.value = titles[page.value] ? page.value : subpages[next]!
  page.value = next
}

const assistantVisited = ref(true)
watch(page, value => { if (value === 'assistant') assistantVisited.value = true })
const content = ref<HTMLElement | null>(null)
watch(page, async () => { await nextTick(); if (content.value) content.value.scrollTop = 0 })

let stop = () => {}
let stopIcon = () => {}
let stopAssistant = () => {}
const assistantOff: (() => void)[] = []
let assistantDisposed = false
async function initAssistant() {
  for (const [name, handler] of [
    ['assistant:open', (value: unknown) => { activeConversation.value = null; assignedPet.value = value as number | null; page.value = 'assistant' }],
    ['assistant:drop', (value: unknown) => { droppedFiles.value = value as string[]; page.value = 'assistant' }],
    ['providers:open', () => { page.value = 'providers' }],
  ] as const) {
    const off = await listen(name, event => handler(event.payload))
    if (assistantDisposed) off(); else assistantOff.push(off)
  }
  if (assistantDisposed) return
  const stop = await startTasks()
  if (assistantDisposed) stop(); else stopAssistant = stop
}

const waiting = computed(() => tasks.value.filter(task => task.status === 'waiting').length)
function followup(task: Task) {
  activeConversation.value = conversationOf(task.id)
  page.value = 'assistant'
}

const status = computed(() => {
  if (!native) return 'Browser preview'
  if (!snapshot.value) return 'Connecting…'
  if (snapshot.value.suspended) return 'Stepped aside'
  if (settings.value?.hidden) return 'Hidden'
  if (settings.value?.paused) return 'Paused'
  if (!pets.value.length) return 'Nobody out yet'
  return pets.value.length === 1 ? '1 pet out' : `${pets.value.length} pets out`
})
const offline = computed(() => !snapshot.value || busy.value)
const quiet = computed(() => !pets.value.length || !!settings.value?.paused || !!settings.value?.hidden || !!snapshot.value?.suspended)

onMounted(() => {
  stop = start()
  if (native) void initAssistant().catch(() => {})
  if (native) stopIcon = syncAppIcon()
})
onUnmounted(() => { stop(); stopIcon(); assistantDisposed = true; stopAssistant(); assistantOff.forEach(off => off()) })
</script>

<template>
  <div class="app-shell">
    <div class="sidebar-shell" :class="{ collapsed, resizing }" :style="{ width: collapsed ? undefined : `${sidebarWidth}px` }">
      <aside class="sidebar">
        <a class="brand" href="#" :title="collapsed ? 'Ledgeling' : undefined" @click.prevent="page = 'assistant'"><span class="brand-mark"><Bird :size="22" :stroke-width="1.7" /></span><span class="label">ledgeling</span></a>
        <nav aria-label="Main navigation">
          <button v-for="item in nav" :key="item.page" :class="{ selected: section === item.page }" :title="collapsed ? item.label : undefined"
                  :aria-current="section === item.page ? 'page' : undefined" @click="page = item.page">
            <component :is="item.icon" :size="18" :stroke-width="1.8" /><span class="label">{{ item.label }}</span>
            <span v-if="item.page === 'history' && waiting" class="nav-badge" :aria-label="`${waiting} waiting for you`">{{ waiting }}</span>
          </button>
        </nav>
        <section v-if="conversations.length && !collapsed" class="chat-section" aria-labelledby="chats-heading">
          <h2 id="chats-heading" class="chat-section-title">Chats</h2>
          <ConversationList :current="page === 'assistant'" @open="page = 'assistant'" />
        </section>
        <div class="sidebar-bottom">
          <span class="sidebar-status" :title="collapsed ? status : undefined"><span class="live-dot" :class="{ sleeping: quiet }" /><span class="label">{{ status }}</span></span>
          <div v-if="pets.length || settings?.paused || settings?.hidden" class="sidebar-controls">
            <button :class="{ active: settings?.paused }" :disabled="offline" :aria-label="settings?.paused ? 'Resume' : 'Pause'" :title="collapsed ? (settings?.paused ? 'Resume' : 'Pause') : undefined" @click="action('pause')">
              <Play v-if="settings?.paused" :size="14" /><Pause v-else :size="14" /><span class="label">{{ settings?.paused ? 'Resume' : 'Pause' }}</span>
            </button>
            <button :class="{ active: settings?.hidden }" :disabled="offline" :aria-label="settings?.hidden ? 'Show' : 'Hide'" :title="collapsed ? (settings?.hidden ? 'Show' : 'Hide') : undefined" @click="action('visibility')">
              <Eye v-if="settings?.hidden" :size="14" /><EyeOff v-else :size="14" /><span class="label">{{ settings?.hidden ? 'Show' : 'Hide' }}</span>
            </button>
          </div>
          <span class="sidebar-private"><ShieldCheck :size="13" /><span class="label">Stays on this computer</span></span>
        </div>
      </aside>
      <div v-if="!collapsed" class="sidebar-resize" role="separator" aria-orientation="vertical" aria-label="Resize sidebar" tabindex="0"
           :aria-valuenow="sidebarWidth" :aria-valuemin="SIDEBAR_MIN" :aria-valuemax="SIDEBAR_MAX"
           @pointerdown.prevent="sidebar.startResize" @dblclick="sidebar.reset" @keydown.left.prevent="sidebar.nudge(-16)" @keydown.right.prevent="sidebar.nudge(16)" />
      <button type="button" class="sidebar-toggle" :aria-label="collapsed ? 'Expand sidebar' : 'Collapse sidebar'" :aria-expanded="!collapsed" @click="collapsed = !collapsed">
        <ChevronRight v-if="collapsed" :size="14" /><ChevronLeft v-else :size="14" />
      </button>
    </div>

    <main ref="content" class="content" :class="{ 'fit-page': page === 'assistant' || page === 'studio' }">
      <div v-if="!native" class="preview-notice"><Monitor :size="16" /><span>This is a browser preview. Run <code>npm run tauri dev</code> to bring a pet onto your desktop.</span></div>
      <div v-for="note in notes" :key="note" class="preview-notice"><ShieldCheck :size="16" /><span>{{ note }}</span></div>
      <div v-if="error || snapshot?.scanError" class="error-banner" role="alert">
        <span>{{ error || snapshot?.scanError }}</span>
        <button v-if="error" aria-label="Dismiss" @click="error = ''"><X :size="16" /></button>
      </div>
      <button v-if="subpages[page]" class="back-link" @click="page = back"><ArrowLeft :size="15" /> {{ titles[back] ?? 'Back' }}</button>

      <div v-if="assistantVisited" v-show="page === 'assistant'" class="assistant-host"><AssistantPanel @providers="page = 'providers'" /></div>
      <ProvidersPanel v-if="page === 'providers'" />
      <HistoryPanel v-else-if="page === 'history'" @followup="followup" />
      <PacksPanel v-else-if="page === 'pets'" @go="go" />
      <SettingsPanel v-else-if="page === 'settings'" @go="go" />
      <DesktopPanel v-else-if="page === 'desktop'" />
      <AwarenessPanel v-else-if="page === 'awareness'" />
      <LabPanel v-else-if="page === 'lab'" />
      <PetStudio v-else-if="page === 'studio'" @go="go" />
    </main>
  </div>
</template>

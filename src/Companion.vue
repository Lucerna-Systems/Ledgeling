<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import AssistantPanel from './assistant/AssistantPanel.vue'
import { activeConversation, conversationOf, conversations, startTasks, assignedPet, droppedFiles, tasks } from './assistant/state'
import ConversationList from './assistant/ConversationList.vue'
import type { Task } from './assistant/types'
import { ArrowLeft, Bird, Bot, ChevronLeft, Code2, ChevronRight, Eye, EyeOff, History, MessageCircle, PawPrint, Trees, Pause, Play, Settings, ShieldCheck, Monitor, X } from '@lucide/vue'
import { action, busy, call, error, native, notes, pets, settings, snapshot, start } from './state'
import PageLoading from './components/PageLoading.vue'
import { syncAppIcon } from './appIcon'
import { companionReady, startPersist, type UiState } from './uiState'
import { draft } from './studio/state'
import { restoreDraft, serializeDraft } from './studio/state'
import { promptDraft } from './assistant/state'
import { SIDEBAR_MAX, SIDEBAR_MIN, useSidebar } from './sidebar'

// Pages other than the default Assistant load on first visit; Vue keeps the loaded component, so revisits are instant.
const lazy = (load: () => Promise<{ default: object }>) => defineAsyncComponent({ loader: load as never, delay: 120, loadingComponent: PageLoading })
const HistoryPanel = lazy(() => import('./assistant/HistoryPanel.vue'))
const ProvidersPanel = lazy(() => import('./assistant/ProvidersPanel.vue'))
const PacksPanel = lazy(() => import('./panels/PacksPanel.vue'))
const SettingsPanel = lazy(() => import('./panels/SettingsPanel.vue'))
const AwarenessPanel = lazy(() => import('./panels/AwarenessPanel.vue'))
const DesktopPanel = lazy(() => import('./panels/DesktopPanel.vue'))
const PetStudio = lazy(() => import('./studio/PetStudio.vue'))
const DecorPanel = lazy(() => import('./panels/DecorPanel.vue'))

type Page = 'pets' | 'decor' | 'assistant' | 'providers' | 'history' | 'settings' | 'desktop' | 'awareness' | 'studio'
const nav = [
  { page: 'assistant', label: 'Assistant', icon: MessageCircle },
  { page: 'providers', label: 'AI providers', icon: Bot },
  { page: 'pets', label: 'Pets', icon: PawPrint },
  { page: 'decor', label: 'Decor', icon: Trees },
  { page: 'history', label: 'History', icon: History },
  { page: 'settings', label: 'Settings', icon: Settings },
] as const
/** Pages reached from inside another section, so the sidebar stays short. */
const subpages: Partial<Record<Page, Page>> = { desktop: 'settings', awareness: 'settings', studio: 'pets' }
const titles: Partial<Record<Page, string>> = { pets: 'Pets', decor: 'Decor', settings: 'Settings', studio: 'Pet Studio' }

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
const handlers: Record<string, (value: unknown) => void> = {
  'assistant:open': value => { activeConversation.value = null; assignedPet.value = value as number | null; page.value = 'assistant' },
  'assistant:drop': value => { droppedFiles.value = value as string[]; page.value = 'assistant' },
  'providers:open': () => { page.value = 'providers' },
}
async function initAssistant() {
  for (const [name, handler] of Object.entries(handlers)) {
    const off = await listen(name, event => handler(event.payload))
    if (assistantDisposed) off(); else assistantOff.push(off)
  }
  if (assistantDisposed) return
  const stop = await startTasks()
  if (assistantDisposed) stop(); else stopAssistant = stop
}

const pages: Page[] = ['pets', 'decor', 'assistant', 'providers', 'history', 'settings', 'desktop', 'awareness', 'studio']
/** Put back what the user was doing before the window was destroyed, then replay events that arrived while it loaded. */
async function restoreAndWatch() {
  const ready = await companionReady()
  const ui = ready.ui
  restoredPage = typeof ui?.page === 'string'
  if (ui) {
    if (typeof ui.prompt === 'string') promptDraft.value = ui.prompt
    const studio = ui.studio ? restoreDraft(ui.studio) : null
    if (studio) draft.value = studio
    if (typeof ui.page === 'string' && (pages as string[]).includes(ui.page) && (ui.page !== 'studio' || studio)) {
      if (subpages[ui.page as Page]) back.value = subpages[ui.page as Page]!
      page.value = ui.page as Page
    }
  }
  for (const { event, payload } of ready.pending) handlers[event]?.(payload)
  if (assistantDisposed) return
  stopPersist = startPersist([page, promptDraft, draft], (): UiState => ({
    page: page.value,
    prompt: promptDraft.value,
    studio: serializeDraft(draft.value),
    volatile: page.value === 'providers',
  }))
}
let stopPersist = () => {}
/** The saved (or first-run) page was applied, so the first-run watch below must not override it. */
let restoredPage = false

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

// A brand-new user's first launch opens on Pets, once, where the tip waits.
let openedFirstRun = false
watch(snapshot, value => {
  if (value?.firstRun && !openedFirstRun && !restoredPage) { openedFirstRun = true; page.value = 'pets' }
})
onMounted(() => {
  stop = start()
  if (native) void initAssistant().then(restoreAndWatch).catch(() => {})
  if (native) stopIcon = syncAppIcon()
})
onUnmounted(() => { stop(); stopPersist(); stopIcon(); assistantDisposed = true; stopAssistant(); assistantOff.forEach(off => off()) })
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
          <button v-if="settings?.developerMode" class="code-entry" :title="collapsed ? 'Coding mode' : 'Open the coding window beside your browser'" :disabled="offline" @click="call('code_open', { petId: null }, false)">
            <Code2 :size="18" :stroke-width="1.8" /><span class="label">Coding mode</span>
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
      <DecorPanel v-else-if="page === 'decor'" />
      <SettingsPanel v-else-if="page === 'settings'" @go="go" />
      <DesktopPanel v-else-if="page === 'desktop'" />
      <AwarenessPanel v-else-if="page === 'awareness'" />
      <PetStudio v-else-if="page === 'studio'" @go="go" />
    </main>
  </div>
</template>

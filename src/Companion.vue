<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import AssistantPanel from './assistant/AssistantPanel.vue'
import HistoryPanel from './assistant/HistoryPanel.vue'
import { startTasks, assignedPet, droppedFiles, pendingFollowup, tasks } from './assistant/state'
import type { Task } from './assistant/types'
import { ArrowLeft, Bird, History, House, MessageCircle, PawPrint, Settings, ShieldCheck, Monitor, X } from '@lucide/vue'
import { error, native, notes, pets, settings, snapshot, start } from './state'
import HomePanel from './panels/HomePanel.vue'
import PacksPanel from './panels/PacksPanel.vue'
import SettingsPanel from './panels/SettingsPanel.vue'
import LabPanel from './panels/LabPanel.vue'
import AwarenessPanel from './panels/AwarenessPanel.vue'
import DesktopPanel from './panels/DesktopPanel.vue'
import PetStudio from './studio/PetStudio.vue'

type Page = 'home' | 'pets' | 'assistant' | 'history' | 'settings' | 'desktop' | 'awareness' | 'lab' | 'studio'
const nav = [
  { page: 'home', label: 'Home', icon: House },
  { page: 'pets', label: 'Pets', icon: PawPrint },
  { page: 'assistant', label: 'Assistant', icon: MessageCircle },
  { page: 'history', label: 'History', icon: History },
  { page: 'settings', label: 'Settings', icon: Settings },
] as const
/** Pages reached from inside another section, so the sidebar stays short. */
const subpages: Partial<Record<Page, Page>> = { desktop: 'settings', awareness: 'settings', lab: 'settings', studio: 'pets' }
const titles: Partial<Record<Page, string>> = { pets: 'Pets', settings: 'Settings', studio: 'Pet Studio' }

const page = ref<Page>('home')
/** Where a sub-page's back button returns to: the page that opened it. */
const back = ref<Page>('settings')
const section = computed(() => subpages[page.value] ?? page.value)
function go(next: Page) {
  if (subpages[next]) back.value = titles[page.value] ? page.value : subpages[next]!
  page.value = next
}

const assistantVisited = ref(false)
watch(page, value => { if (value === 'assistant') assistantVisited.value = true })
const content = ref<HTMLElement | null>(null)
watch(page, async () => { await nextTick(); if (content.value) content.value.scrollTop = 0 })

let stop = () => {}
let stopAssistant = () => {}
const assistantOff: (() => void)[] = []
let assistantDisposed = false
async function initAssistant() {
  for (const [name, handler] of [
    ['assistant:open', (value: unknown) => { assignedPet.value = value as number | null; page.value = 'assistant' }],
    ['assistant:drop', (value: unknown) => { droppedFiles.value = value as string[]; page.value = 'assistant' }],
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
  pendingFollowup.value = task
  page.value = 'assistant'
}

function ask(petId: number) {
  assignedPet.value = petId
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
const quiet = computed(() => !pets.value.length || !!settings.value?.paused || !!settings.value?.hidden || !!snapshot.value?.suspended)

onMounted(() => {
  stop = start()
  if (native) void initAssistant().catch(() => {})
})
onUnmounted(() => { stop(); assistantDisposed = true; stopAssistant(); assistantOff.forEach(off => off()) })
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <a class="brand" href="#" @click.prevent="page = 'home'"><span class="brand-mark"><Bird :size="22" :stroke-width="1.7" /></span>perch</a>
      <nav aria-label="Main navigation">
        <button v-for="item in nav" :key="item.page" :class="{ selected: section === item.page }"
                :aria-current="section === item.page ? 'page' : undefined" @click="page = item.page">
          <component :is="item.icon" :size="18" :stroke-width="1.8" /> {{ item.label }}
          <span v-if="item.page === 'history' && waiting" class="nav-badge" :aria-label="`${waiting} waiting for you`">{{ waiting }}</span>
        </button>
      </nav>
      <div class="sidebar-bottom">
        <span class="sidebar-status"><span class="live-dot" :class="{ sleeping: quiet }" /> {{ status }}</span>
        <span class="sidebar-private"><ShieldCheck :size="13" /> Stays on this computer</span>
      </div>
    </aside>

    <main ref="content" class="content" :class="{ 'fit-page': page === 'assistant' }">
      <div v-if="!native" class="preview-notice"><Monitor :size="16" /><span>This is a browser preview. Run <code>npm run tauri dev</code> to bring a pet onto your desktop.</span></div>
      <div v-for="note in notes" :key="note" class="preview-notice"><ShieldCheck :size="16" /><span>{{ note }}</span></div>
      <div v-if="error || snapshot?.scanError" class="error-banner" role="alert">
        <span>{{ error || snapshot?.scanError }}</span>
        <button v-if="error" aria-label="Dismiss" @click="error = ''"><X :size="16" /></button>
      </div>
      <button v-if="subpages[page]" class="back-link" @click="page = back"><ArrowLeft :size="15" /> {{ titles[back] ?? 'Back' }}</button>

      <HomePanel v-if="page === 'home'" @go="go" @ask="ask" />
      <div v-if="assistantVisited" v-show="page === 'assistant'" class="assistant-host"><AssistantPanel /></div>
      <HistoryPanel v-if="page === 'history'" @followup="followup" />
      <PacksPanel v-else-if="page === 'pets'" @go="go" />
      <SettingsPanel v-else-if="page === 'settings'" @go="go" />
      <DesktopPanel v-else-if="page === 'desktop'" />
      <AwarenessPanel v-else-if="page === 'awareness'" />
      <LabPanel v-else-if="page === 'lab'" />
      <PetStudio v-else-if="page === 'studio'" @go="go" />
    </main>
  </div>
</template>

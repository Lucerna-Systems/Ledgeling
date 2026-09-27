<script setup lang="ts">
import { onMounted } from 'vue'
import { Check, Download, ExternalLink, PackagePlus, RefreshCw } from '@lucide/vue'
import { code, root, tab } from '../state'
import { loadTools, tools } from '../tools'
import { pendingTerminal } from '../terminalFocus'

onMounted(() => void loadTools())

/** Installing runs the bundled script in a Terminal tab you can watch. */
async function install(id: string) {
  pendingTerminal.value = { tool: id, worktree: root.value ?? '', title: `Install ${id}` }
  tab.value = 'terminal'
}
async function openHome(url: string) {
  await code('code_open_url', { url }).catch(() => {})
}
</script>

<template>
  <div class="section-title"><PackagePlus :size="12" /> Add-ons <span style="flex:1" /><button class="icon" title="Check again" @click="loadTools(true)"><RefreshCw :size="12" /></button></div>
  <div class="cards">
    <article v-for="tool in tools" :key="tool.id" class="card">
      <div class="card-head">
        <div class="grow">
          <strong>{{ tool.name }}</strong>
          <small>{{ tool.purpose }}</small>
        </div>
        <span v-if="tool.version" class="chip live" :title="tool.version"><Check :size="11" /> Installed</span>
        <template v-else>
          <button v-if="tool.installer" class="primary-button" :title="`Runs ${tool.installer} in a terminal`" @click="install(tool.id)"><Download :size="13" /> Install</button>
          <button v-else class="ghost" @click="openHome(tool.homepage)"><ExternalLink :size="13" /> Get it</button>
        </template>
      </div>
      <p v-if="!tool.version && tool.installer" class="muted small mono" style="margin-top:6px;overflow-wrap:anywhere">{{ tool.installer }}</p>
    </article>
  </div>
  <p class="muted small" style="padding: 0 12px 14px">Nothing is installed until you press Install, and you can watch every step in the Terminal tab.</p>
</template>

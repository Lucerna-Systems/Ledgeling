<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { ArrowLeft, CloudUpload, GitPullRequest, LoaderCircle, X } from '@lucide/vue'
import GhMarkdown from './GhMarkdown.vue'
import { ask, composing, follow, refreshLists, repo, show } from './store'
import type { PrDraft } from './types'

/** A pull request from the worktree's branch, drafted from its commits. */
const draft = ref<PrDraft | null>(null)
const failed = ref('')
const title = ref('')
const body = ref('')
const base = ref('')
const asDraft = ref(false)
const preview = ref(false)
const busy = ref(false)

onMounted(async () => {
  try {
    const d = await ask<PrDraft>('code_gh_pr_draft')
    draft.value = d
    title.value = d.title
    body.value = d.body
    base.value = d.base
  } catch (e) { failed.value = String(e) }
})

const cancel = () => { composing.value = null }

async function create() {
  if (!title.value.trim() || busy.value) return
  busy.value = true
  try {
    const url = await ask<string>('code_gh_create_pr', { base: base.value, title: title.value, body: body.value, draft: asDraft.value })
    const number = Number(/\/pull\/(\d+)/.exec(url)?.[1])
    refreshLists()
    if (number) show('pr', number)
    else composing.value = null
  } catch { /* shown above */ } finally { busy.value = false }
}
</script>

<template>
  <form class="gh-compose" @submit.prevent="create">
    <header class="gh-compose-head">
      <GitPullRequest :size="15" class="gh-state-icon open" />
      <h1>New pull request</h1>
      <span class="grow" />
      <button type="button" class="icon" title="Cancel" @click="cancel"><X :size="14" /></button>
    </header>
    <div v-if="failed" class="banner warn"><p>{{ failed }}</p></div>
    <div v-else-if="!draft" class="gh-loading"><LoaderCircle :size="16" class="spin" /> Reading your branch…</div>
    <template v-else>
      <div class="gh-branches">
        <label>into <input v-model="base" class="gh-ref-input mono" aria-label="Base branch" spellcheck="false"></label>
        <ArrowLeft :size="14" class="muted" />
        <span class="gh-ref">{{ draft.branch }}</span>
        <span class="muted small">{{ draft.commits }} commit{{ draft.commits === 1 ? '' : 's' }}</span>
      </div>
      <p v-if="!draft.pushed" class="gh-push-note"><CloudUpload :size="13" /> {{ draft.branch }} will be pushed to {{ draft.remote }} first.</p>
      <input v-model="title" v-focus class="gh-title-input" placeholder="Title" aria-label="Title" required>
      <div class="gh-composer-tabs">
        <span class="segmented-mini" role="group" aria-label="Description view">
          <button type="button" :aria-pressed="!preview" @click="preview = false">Write</button>
          <button type="button" :aria-pressed="preview" @click="preview = true">Preview</button>
        </span>
      </div>
      <GhMarkdown v-if="preview" class="gh-preview tall" :text="body" :repo-url="repo?.url" @link="follow" />
      <textarea v-else v-model="body" class="gh-textarea tall" placeholder="What does this change, and why?" aria-label="Description" @keydown.meta.enter.prevent="create" @keydown.ctrl.enter.prevent="create" />
      <div class="gh-composer-actions">
        <label class="gh-check-label"><input v-model="asDraft" type="checkbox"> Open as a draft</label>
        <span class="grow" />
        <button type="button" class="ghost" @click="cancel">Cancel</button>
        <button type="submit" class="primary-button" :disabled="!title.trim() || !base.trim() || busy">
          <LoaderCircle v-if="busy" :size="13" class="spin" />{{ draft.pushed ? 'Create pull request' : 'Push and create' }}
        </button>
      </div>
    </template>
  </form>
</template>

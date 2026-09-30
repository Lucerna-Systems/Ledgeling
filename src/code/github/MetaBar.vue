<script setup lang="ts">
import { computed, ref } from 'vue'
import { Milestone, Plus, Tag, Users } from '@lucide/vue'
import Avatar from './Avatar.vue'
import Picker from './Picker.vue'
import { labelStyle } from './format'
import { act, canWrite, gh, loadMeta, meta } from './store'
import type { Actor, Kind, Label } from './types'

/** Labels and assignees, editable in place, plus reviewers for pull requests. */
const props = defineProps<{ kind: Kind; number: number; labels: Label[]; assignees: Actor[]; reviewers?: string[]; milestone?: string | null }>()
const emit = defineEmits<{ changed: [] }>()
const picking = ref<'labels' | 'assignees' | null>(null)

async function pick(which: 'labels' | 'assignees') {
  await loadMeta()
  picking.value = which
}
const labelOptions = computed(() => (meta.value?.labels ?? []).map(l => ({ value: l.name, color: l.color, hint: l.description })))
const peopleOptions = computed(() => {
  const known = new Set(meta.value?.assignees ?? [])
  if (gh.value?.account) known.add(gh.value.account)
  return [...known].map(value => ({ value, hint: value === gh.value?.account ? 'You' : undefined }))
})
async function apply(which: 'labels' | 'assignees', add: string[], remove: string[]) {
  try {
    await act({ action: which, kind: props.kind, number: props.number, add, remove })
    emit('changed')
  } catch { /* shown above */ }
}
const assignSelf = () => gh.value?.account && apply('assignees', [gh.value.account], [])
</script>

<template>
  <div class="gh-meta">
    <div class="gh-meta-row">
      <span class="gh-meta-key"><Tag :size="12" /> Labels</span>
      <span class="gh-meta-values">
        <span v-for="label in labels" :key="label.name" class="gh-label" :style="labelStyle(label.color)">{{ label.name }}</span>
        <span v-if="!labels.length" class="muted">None</span>
        <button v-if="canWrite" class="icon gh-meta-edit" title="Edit labels" @click="pick('labels')"><Plus :size="12" /></button>
      </span>
    </div>
    <div class="gh-meta-row">
      <span class="gh-meta-key"><Users :size="12" /> Assignees</span>
      <span class="gh-meta-values">
        <span v-for="a in assignees" :key="a.login" class="gh-person"><Avatar :login="a.login" :size="16" />{{ a.login }}</span>
        <template v-if="!assignees.length">
          <span class="muted">No one</span>
          <button v-if="canWrite && gh?.account" class="bare text-link gh-link" @click="assignSelf">assign yourself</button>
        </template>
        <button v-if="canWrite" class="icon gh-meta-edit" title="Edit assignees" @click="pick('assignees')"><Plus :size="12" /></button>
      </span>
    </div>
    <div v-if="reviewers?.length" class="gh-meta-row">
      <span class="gh-meta-key"><Users :size="12" /> Reviewers</span>
      <span class="gh-meta-values"><span v-for="r in reviewers" :key="r" class="gh-person"><Avatar :login="r" :size="16" />{{ r }}</span></span>
    </div>
    <div v-if="milestone" class="gh-meta-row">
      <span class="gh-meta-key"><Milestone :size="12" /> Milestone</span>
      <span class="gh-meta-values">{{ milestone }}</span>
    </div>
    <Picker v-if="picking === 'labels'" title="Labels" :options="labelOptions" :selected="labels.map(l => l.name)" @apply="(a, r) => apply('labels', a, r)" @close="picking = null" />
    <Picker v-if="picking === 'assignees'" title="Assignees" people :options="peopleOptions" :selected="assignees.map(a => a.login)" @apply="(a, r) => apply('assignees', a, r)" @close="picking = null" />
  </div>
</template>

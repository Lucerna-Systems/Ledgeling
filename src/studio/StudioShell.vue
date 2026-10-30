<script setup lang="ts">
import { AlertCircle, Check } from '@lucide/vue'
import './studio.css'

/** The frame every Studio shares: who made what (name, and optionally author
 *  and a description), the save buttons, and one status line that is either
 *  "saved" or the first thing still stopping a save. What is being made goes
 *  in the default slot; tabs or a mode bar go in `tabs`. */
withDefaults(defineProps<{
  nameLabel: string
  namePlaceholder: string
  /** Shown only when given; leave it out to hide the field. */
  authorPlaceholder?: string
  descriptionPlaceholder?: string
  /** Everything that stops a save, most important first. */
  issues: string[]
  /** Shows the `saved` slot in place of the issues. */
  saved?: boolean
}>(), { authorPlaceholder: undefined, descriptionPlaceholder: undefined, saved: false })
const name = defineModel<string>('name', { required: true })
const author = defineModel<string>('author')
const description = defineModel<string>('description')
</script>

<template>
  <section class="studio">
    <header class="studio-head">
      <div class="studio-identity">
        <input v-model="name" class="studio-name" :aria-label="nameLabel" :placeholder="namePlaceholder" maxlength="40" spellcheck="false">
        <div v-if="authorPlaceholder || descriptionPlaceholder" class="studio-meta">
          <input v-if="authorPlaceholder" v-model="author" aria-label="Made by" :placeholder="authorPlaceholder" maxlength="40">
          <input v-if="descriptionPlaceholder" v-model="description" class="grow" aria-label="Description" :placeholder="descriptionPlaceholder" maxlength="140">
        </div>
      </div>
      <div class="studio-actions"><slot name="actions" /></div>
    </header>

    <slot name="tabs" />

    <div v-if="saved" class="studio-saved" role="status">
      <Check :size="16" /> <slot name="saved" />
    </div>
    <div v-else-if="issues.length" class="studio-issues">
      <AlertCircle :size="15" />
      <span>Before saving: {{ issues[0] }}<template v-if="issues.length > 1"> <em>(+{{ issues.length - 1 }} more)</em></template></span>
    </div>

    <slot />
  </section>
</template>

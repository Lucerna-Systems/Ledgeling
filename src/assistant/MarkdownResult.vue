<script setup lang="ts">
import { ref, watchEffect } from 'vue'

const props = defineProps<{ text: string }>()
const emit = defineEmits<{ open: [url: string] }>()
const escape = (text: string) => text.replace(/[&<>"]/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[ch]!)
// marked and DOMPurify load on first render; until then (a frame or two) the text shows as plain escaped text.
const html = ref(`<p>${escape(props.text)}</p>`)
let render: ((text: string) => string) | null = null
const loading = import('./markdown').then(module => { render = module.renderMarkdown })
watchEffect(() => {
  const text = props.text
  if (render) html.value = render(text)
  else void loading.then(() => { if (render && text === props.text) html.value = render(text) })
})
function openLink(event: MouseEvent) {
  const link = (event.target as Element).closest('a')
  if (!link) return
  event.preventDefault()
  const url = link.getAttribute('href') ?? ''
  if (/^https?:\/\//i.test(url)) emit('open', url)
}
</script>
<template>
  <div class="markdown-body" @click="openLink" @auxclick.prevent v-html="html" />
</template>

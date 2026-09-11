<script setup lang="ts">
import { computed } from 'vue'
import { marked } from 'marked'
import DOMPurify from 'dompurify'

const props = defineProps<{ text: string }>()
const emit = defineEmits<{ open: [url: string] }>()
const html = computed(() => DOMPurify.sanitize(marked.parse(props.text, { async: false }), {
  ALLOWED_TAGS: ['p', 'br', 'strong', 'em', 'del', 'a', 'ul', 'ol', 'li', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'blockquote', 'pre', 'code', 'hr', 'table', 'thead', 'tbody', 'tr', 'th', 'td'],
  ALLOWED_ATTR: ['href', 'title', 'start'],
  ALLOWED_URI_REGEXP: /^https?:\/\//i,
  ALLOW_DATA_ATTR: false,
}))
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

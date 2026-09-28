<script setup lang="ts">
import { computed } from 'vue'
import { Marked } from 'marked'
import DOMPurify from 'dompurify'
import { linkRefs } from './format'

/** GitHub-flavoured Markdown from issues and pull requests. Images become
 *  links, since the window loads nothing from the network. */
const props = defineProps<{ text: string; repoUrl?: string }>()
const emit = defineEmits<{ link: [url: string] }>()

const escape = (text: string) => text.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!)
const marked = new Marked({
  gfm: true,
  breaks: true,
  renderer: {
    image({ href, text }) { return `<a href="${escape(href)}" class="gh-image-link">🖼 ${escape(text || 'Image')}</a>` },
  },
})

const html = computed(() => {
  const source = props.text?.trim() ? props.text : ''
  if (!source) return ''
  const linked = props.repoUrl ? linkRefs(source, props.repoUrl) : source
  return DOMPurify.sanitize(marked.parse(linked, { async: false }) as string, {
    ALLOWED_TAGS: ['p', 'br', 'strong', 'em', 'del', 's', 'a', 'ul', 'ol', 'li', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'blockquote', 'pre', 'code', 'hr', 'table', 'thead', 'tbody', 'tr', 'th', 'td', 'details', 'summary', 'input', 'kbd', 'sup', 'sub'],
    ALLOWED_ATTR: ['href', 'title', 'start', 'type', 'checked', 'disabled', 'class', 'open'],
    ALLOWED_URI_REGEXP: /^https?:\/\//i,
    ALLOW_DATA_ATTR: false,
  })
})

function click(event: MouseEvent) {
  const link = (event.target as Element).closest('a')
  if (!link) return
  event.preventDefault()
  const url = link.getAttribute('href') ?? ''
  if (/^https?:\/\//i.test(url)) emit('link', url)
}
</script>

<template>
  <div v-if="html" class="gh-md" @click="click" @auxclick.prevent v-html="html" />
  <p v-else class="gh-md muted"><em>No description provided.</em></p>
</template>

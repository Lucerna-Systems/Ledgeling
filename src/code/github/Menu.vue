<script setup lang="ts">
import { ref } from 'vue'
import { ChevronDown } from '@lucide/vue'
import type { MenuItem } from './types'

/** A button with a dropdown of further actions. */
defineProps<{ items: MenuItem[]; label?: string; title?: string; align?: 'left' | 'right' }>()
const shown = ref(false)
function pick(item: MenuItem) {
  shown.value = false
  void item.run()
}
</script>

<template>
  <span class="gh-menu">
    <button :class="label ? 'ghost' : 'icon'" :title="title ?? label" :aria-expanded="shown" aria-haspopup="menu" @click="shown = !shown">
      <slot />{{ label }}<ChevronDown :size="12" />
    </button>
    <template v-if="shown">
      <div class="popover-scrim" @click="shown = false" />
      <div class="gh-menu-list" :class="align ?? 'right'" role="menu" @keydown.esc="shown = false">
        <button v-for="item in items" :key="item.label" class="bare" role="menuitem" :class="{ danger: item.danger }" :disabled="item.disabled" @click="pick(item)">
          <component :is="item.icon" v-if="item.icon" :size="14" />
          <span><strong>{{ item.label }}</strong><small v-if="item.hint">{{ item.hint }}</small></span>
        </button>
      </div>
    </template>
  </span>
</template>

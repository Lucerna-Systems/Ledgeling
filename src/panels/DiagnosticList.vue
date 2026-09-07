<script setup lang="ts">
import type { Report } from '../types'
defineProps<{ report: Report }>()
</script>
<template>
  <ul class="diagnostics">
    <li v-for="(d, i) in report.diagnostics" :key="i" :class="d.tier">
      <span class="diagnostic-tier">{{ d.tier === 'warning' ? 'WARN' : d.tier.toUpperCase() }}</span>
      <div>
        <code>{{ d.file }}<template v-if="d.path"> → {{ d.path }}</template></code>
        <p>{{ d.problem }}</p>
        <p class="expected">{{ d.expected }}</p>
      </div>
    </li>
    <li v-if="!report.diagnostics.length" class="info">
      <span class="diagnostic-tier">OK</span>
      <div><p>Nothing to report. This pack is valid.</p></div>
    </li>
  </ul>
</template>

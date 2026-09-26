import { ref } from 'vue'
import { starterDraft, type Draft } from './model'

/** The design in progress; module-level so leaving the Studio and coming back keeps the work. */
export const draft = ref<Draft>(starterDraft())

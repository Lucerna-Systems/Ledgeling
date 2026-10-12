import { computed } from 'vue'
import { snapshot, pets, error, busy, native, action } from '../state'

export function useWorldSnapshot() {
  const currentSnapshot = computed(() => snapshot.value)
  const allPets = computed(() => pets.value)
  const isSuspended = computed(() => !!snapshot.value?.suspended)
  const scanError = computed(() => snapshot.value?.scanError ?? '')

  return {
    snapshot: currentSnapshot,
    pets: allPets,
    error,
    busy,
    native,
    isSuspended,
    scanError,
    triggerAction: action,
  }
}

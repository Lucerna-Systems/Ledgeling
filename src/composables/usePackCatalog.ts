import { computed } from 'vue'
import { packs, refreshPacks, validate, call } from '../state'
import type { PackListing, PackSummary, Report } from '../types'

export function usePackCatalog() {
  const allPacks = computed<PackListing[]>(() => packs.value)

  async function loadPacks() {
    await refreshPacks()
  }

  async function validatePath(path: string): Promise<Report> {
    return await validate(path)
  }

  async function installPack(path: string): Promise<PackSummary | undefined> {
    return await call<PackSummary>('install_pack', { path })
  }

  async function uninstallPack(id: string): Promise<void> {
    await call<void>('uninstall_pack', { id })
  }

  return {
    packs: allPacks,
    loadPacks,
    validatePath,
    installPack,
    uninstallPack,
  }
}

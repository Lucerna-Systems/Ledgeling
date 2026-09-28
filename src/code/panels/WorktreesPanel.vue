<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ExternalLink, FolderGit2, FolderOpen, FolderPlus, GitBranchPlus, Trash2, X } from '@lucide/vue'
import PetFace from '../../assistant/PetFace.vue'
import AddonsSheet from '../components/AddonsSheet.vue'
import { pets } from '../../state'
import { activityOf, code, codeState, current, petFor, refreshState, select, tab, wide } from '../state'
import type { GitStatus, ProjectView, WorktreeView } from '../types'
import { ask } from '../confirm'
import { say } from '../petNotes'

const props = defineProps<{ active?: boolean }>()
const statuses = reactive(new Map<string, GitStatus | null>())
const creating = ref<ProjectView | null>(null)
const branch = ref('')
const base = ref('')
const branches = ref<string[]>([])
const petChoice = ref<number | null>(null)
const busy = ref(false)
const assigning = ref<string | null>(null)

async function loadStatuses() {
  for (const project of codeState.value.projects) {
    for (const tree of project.worktrees) {
      code<GitStatus>('code_status', { worktree: tree.path }).then(s => statuses.set(tree.path, s)).catch(() => statuses.set(tree.path, null))
    }
  }
}
watch(() => props.active, now => { if (now) void loadStatuses() }, { immediate: true })

const taken = computed(() => new Set(codeState.value.projects.flatMap(p => p.worktrees.map(w => w.petId)).filter((id): id is number => id != null)))

async function startCreate(project: ProjectView) {
  creating.value = project
  branch.value = ''
  petChoice.value = pets.value.find(p => !taken.value.has(p.id))?.id ?? null
  try {
    branches.value = await code<string[]>('code_branches', { project: project.path })
    const main = project.worktrees.find(w => w.main)?.branch
    base.value = main ?? branches.value[0] ?? ''
  } catch { branches.value = [] }
}
async function create() {
  const project = creating.value
  if (!project || !branch.value.trim()) return
  busy.value = true
  try {
    const path = await code<string>('code_worktree_create', { project: project.path, branch: branch.value.trim(), base: base.value, petId: petChoice.value })
    const pet = pets.value.find(p => p.id === petChoice.value)
    if (pet) void say(path, `Hi, I'm ${pet.name}. I'll look after ${branch.value.trim()}, started from ${base.value}, and tell you what happens here.`)
    creating.value = null
    await refreshState()
    void loadStatuses()
  } catch { /* banner */ } finally { busy.value = false }
}
async function remove(project: ProjectView, tree: WorktreeView) {
  const s = statuses.get(tree.path)
  const dirty = (s?.files.length ?? 0) > 0
  const text = dirty
    ? `${tree.name} has ${s!.files.length} uncommitted change${s!.files.length === 1 ? '' : 's'}. Remove it and lose them? Its branch ${tree.branch ?? ''} is kept.`
    : `Remove the worktree ${tree.name}? Its folder is deleted; the branch ${tree.branch ?? ''} is kept.`
  if (!await ask(text, 'Remove worktree')) return
  try { await code('code_worktree_remove', { project: project.path, path: tree.path, force: dirty }); await refreshState() } catch { /* banner */ }
}
async function assign(tree: WorktreeView, petId: number | null) {
  assigning.value = null
  try {
    await code('code_bind', { worktree: tree.path, petId })
    const pet = pets.value.find(p => p.id === petId)
    if (pet) void say(tree.path, `I'm ${pet.name}, looking after ${tree.name} now.`)
    await refreshState()
  } catch { /* banner */ }
}
async function removeProject(project: ProjectView) {
  if (!await ask(`Stop showing ${project.name} here? Nothing on disk changes.`, 'Remove from list', false)) return
  await code('code_remove_project', { path: project.path }).catch(() => {})
  await refreshState()
}
async function addProject() {
  try { await code('code_add_project'); await refreshState() } catch { /* banner */ }
}
async function openIn(tree: WorktreeView, target: 'here' | 'finder') {
  if (target === 'here') { await select(tree.path); tab.value = wide.value ? 'terminal' : 'files'; return }
  await code('code_file_op', { worktree: tree.path, op: 'reveal', path: '', to: '' }).catch(() => {})
}
function describe(s: GitStatus | null | undefined) {
  if (!s) return null
  const parts = [s.files.length ? `${s.files.length} changed` : 'clean']
  if (s.ahead) parts.push(`${s.ahead} ahead`)
  if (s.behind) parts.push(`${s.behind} behind`)
  return parts.join(' · ')
}
</script>

<template>
  <section>
    <div class="panel-bar">
      <h2 class="grow">Worktrees</h2>
      <button class="ghost" @click="addProject"><FolderPlus :size="14" /> Add project</button>
    </div>
    <div class="panel-scroll">
      <p class="muted small" style="padding: 10px 12px 2px">Each worktree is a separate copy of your project on its own branch. Give each one a pet, so you always know who is working where.</p>
      <template v-for="project in codeState.projects" :key="project.path">
        <div class="section-title">
          <FolderGit2 :size="12" /> {{ project.name }}
          <span style="flex:1" />
          <button class="icon" :title="`New worktree in ${project.name}`" @click="startCreate(project)"><GitBranchPlus :size="14" /></button>
          <button class="icon" :title="`Remove ${project.name} from this list`" @click="removeProject(project)"><X :size="13" /></button>
        </div>
        <p v-if="project.problem" class="banner warn"><span>{{ project.problem }}</span></p>

        <div v-if="creating?.path === project.path" class="cards">
          <form class="card form" @submit.prevent="create">
            <h3>New worktree</h3>
            <div class="pair">
              <label>Branch name<input v-model="branch" v-focus required placeholder="e.g. fix-login" pattern="[A-Za-z0-9._/\-]+" /></label>
              <label>Start from<select v-model="base"><option v-for="b in branches" :key="b" :value="b">{{ b }}</option></select></label>
            </div>
            <label>Pet working here</label>
            <div class="pet-pick" role="radiogroup" aria-label="Pet">
              <button v-for="pet in pets" :key="pet.id" type="button" :aria-pressed="petChoice === pet.id" :title="taken.has(pet.id) ? `${pet.name} will move here from its current worktree` : pet.name" @click="petChoice = pet.id">
                <PetFace :pet="{ packId: pet.packId, name: pet.name }" :size="22" /> {{ pet.name }}<span v-if="taken.has(pet.id)" class="muted small">(busy)</span>
              </button>
              <button type="button" :aria-pressed="petChoice === null" @click="petChoice = null">No pet</button>
            </div>
            <div class="card-actions">
              <button class="primary-button" type="submit" :disabled="busy || !branch.trim()"><GitBranchPlus :size="14" /> Create</button>
              <button type="button" class="ghost" @click="creating = null">Cancel</button>
            </div>
          </form>
        </div>

        <div class="cards">
          <article v-for="tree in project.worktrees" :key="tree.path" class="card" :class="{ current: tree.path === current?.worktree.path }">
            <div class="card-head">
              <button class="bare" :title="petFor(tree) ? `${petFor(tree)!.name} works here. Change pet` : 'Give this worktree a pet'" @click="assigning = assigning === tree.path ? null : tree.path">
                <PetFace :pet="petFor(tree) ? { packId: petFor(tree)!.packId, name: petFor(tree)!.name } : null" :pet-id="tree.petId" :size="34" />
              </button>
              <div class="grow">
                <strong>{{ tree.name }}</strong>
                <small><FolderGit2 :size="11" /> {{ tree.branch ?? 'detached' }} · {{ petFor(tree)?.name ?? 'no pet' }}</small>
              </div>
              <span v-if="activityOf(tree.path)" class="chip" :class="activityOf(tree.path)!.tone">{{ activityOf(tree.path)!.label }}</span>
            </div>
            <div class="card-meta">
              <span v-if="tree.main" class="chip accent">Main checkout</span>
              <span v-if="tree.path === current?.worktree.path" class="chip accent">On screen</span>
              <span v-if="describe(statuses.get(tree.path))" class="chip">{{ describe(statuses.get(tree.path)) }}</span>
              <span v-if="tree.locked" class="chip">Locked</span>
            </div>
            <div v-if="assigning === tree.path" class="pet-pick" style="margin-top:10px" role="radiogroup" aria-label="Pet for this worktree">
              <button v-for="pet in pets" :key="pet.id" :aria-pressed="tree.petId === pet.id" @click="assign(tree, pet.id)">
                <PetFace :pet="{ packId: pet.packId, name: pet.name }" :size="22" /> {{ pet.name }}
              </button>
              <button :aria-pressed="tree.petId === null" @click="assign(tree, null)">No pet</button>
            </div>
            <div class="card-actions">
              <button v-if="tree.path !== current?.worktree.path" class="primary-button" @click="openIn(tree, 'here')"><FolderOpen :size="13" /> Open</button>
              <button class="ghost" @click="openIn(tree, 'finder')"><ExternalLink :size="13" /> Show folder</button>
              <button v-if="!tree.main" class="ghost danger-button" @click="remove(project, tree)"><Trash2 :size="13" /> Remove</button>
            </div>
          </article>
        </div>
      </template>
      <AddonsSheet />
    </div>
  </section>
</template>

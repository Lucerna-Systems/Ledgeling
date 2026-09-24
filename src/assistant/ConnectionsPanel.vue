<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import SearchSelect from '../components/SearchSelect.vue'
import { pets } from '../state'
import { connections, effectiveConnection, taskCall } from './state'
import type { Capabilities, Connection, ProviderDetection, ProviderKind } from './types'
const busy = ref(false)
const feedback = ref('')
const key = ref('')
const braveKey = ref('')
const detection = ref<ProviderDetection | null>(null)
const models = ref<string[]>([])
const loadingModels = ref(false)
const accountNote = ref('')
const loginUrl = ref('')
/** Perch's own terms for the on-device model, served by the backend so the
 *  version shown is always the version the gate checks. */
const appleTerms = ref<{ version: number; text: string; accepted: boolean } | null>(null)
const appleAgreed = ref(false)
const appleTermsOpen = ref(false)
// Selecting Apple is not enough; the terms gate is enforced in the backend too.
const appleBlocked = computed(() => active.value.kind === 'apple' && !appleAgreed.value)
async function loadAppleTerms() {
  try {
    appleTerms.value = await taskCall<{ version: number; text: string; accepted: boolean }>('task_apple_terms')
    appleAgreed.value = appleTerms.value.accepted
  } catch { /* shared error */ }
}
async function openAppleSettings() {
  try { await taskCall('task_apple_open_settings') } catch { /* shared error */ }
}
const defaults: Record<ProviderKind, Partial<Connection>> = {
  none: {}, ollama: { endpoint: 'http://127.0.0.1:11434' }, openai: {}, anthropic: {},
  compatible: { endpoint: 'https://your-provider.example/v1' }, codex: { executable: 'codex' }, claude: {},
  apple: { model: 'system' },
}
const installedKinds: ProviderKind[] = ['none', 'apple', 'codex', 'claude', 'ollama']
const active = computed(() => connections.value.active)
function choose(kind: ProviderKind) {
  connections.value.active = { kind, model: '', endpoint: '', executable: '', ...defaults[kind] }
  feedback.value = ''; key.value = ''; loginUrl.value = ''; accountNote.value = ''
  loadModels(); checkStatus()
}
const installedChoice = computed({
  get: () => (installedKinds.includes(active.value.kind) ? active.value.kind : ''),
  set: kind => choose((kind || 'none') as ProviderKind),
})
const apiChoice = computed({
  get: () => (installedKinds.includes(active.value.kind) ? '' : active.value.kind),
  set: kind => choose((kind || 'none') as ProviderKind),
})
// A saved provider stays selectable even when detection misses it, so a
// custom executable path or a stopped Ollama cannot lock the user out.
const showCodex = computed(() => detection.value?.codexInstalled || active.value.kind === 'codex')
const showClaude = computed(() => detection.value?.claudeBridge || active.value.kind === 'claude')
const showOllama = computed(() => detection.value?.ollamaApp || detection.value?.ollamaReachable || active.value.kind === 'ollama')
// Offered on any Mac that could run it, so the reason it is not ready is
// visible rather than the option silently missing.
const showApple = computed(() => detection.value?.appleSupported || active.value.kind === 'apple')
const providerStatus = computed(() => {
  const d = detection.value
  if (!d) return ''
  switch (active.value.kind) {
    case 'apple': return d.appleDetail
    case 'codex': return d.codexInstalled ? 'Codex was found on this computer.' : 'Codex was not found — check the executable path below.'
    case 'claude': return d.anthropicKey ? 'Claude runtime included with Perch. Anthropic key saved.' : 'Claude runtime included with Perch. Add an Anthropic key below.'
    case 'ollama': return d.ollamaReachable ? 'Ollama is running.' : d.ollamaApp ? 'Ollama is installed but not running.' : 'Ollama was not detected on this computer.'
    case 'openai': return d.openaiKey ? 'OpenAI key is saved in your OS credential store.' : 'No OpenAI key saved yet — add one below.'
    case 'anthropic': return d.anthropicKey ? 'Anthropic key is saved in your OS credential store.' : 'No Anthropic key saved yet — add one below.'
    case 'compatible': return d.compatibleKey ? 'Endpoint key is saved in your OS credential store.' : 'No key saved for this endpoint yet — add one below if it needs one.'
    default: return ''
  }
})
const modelPlaceholder = computed(() => loadingModels.value ? 'Listing models…' : 'Search or type a model ID')
async function loadModels() {
  models.value = []
  if (active.value.kind === 'none') return
  loadingModels.value = true
  // A failed listing (no key yet, provider offline) still leaves free-text entry.
  try { models.value = await taskCall<string[]>('task_models', { config: active.value }) } catch { /* shared error */ } finally { loadingModels.value = false }
}
async function checkStatus() {
  accountNote.value = ''
  if (active.value.kind !== 'codex' || !detection.value?.codexInstalled) return
  try {
    const result = await taskCall<{ account?: { email?: string; name?: string; accountName?: string } }>('task_account', { config: active.value, action: 'status' })
    const who = result.account ? (result.account.email ?? result.account.name ?? result.account.accountName) : undefined
    accountNote.value = who ? `Signed in to ChatGPT as ${who}.` : 'Not signed in to ChatGPT yet.'
  } catch { /* shared error */ }
}
async function save() {
  busy.value = true
  try {
    const kind = active.value.kind
    if (key.value) await taskCall('task_secret', { name: kind === 'claude' ? 'anthropic' : kind, value: key.value })
    if (braveKey.value) await taskCall('task_secret', { name: 'brave', value: braveKey.value })
    connections.value.appleTerms = { acceptedVersion: appleAgreed.value ? (appleTerms.value?.version ?? 0) : 0, acceptedAt: 0 }
    await taskCall('task_save_connections', { config: connections.value })
    await loadAppleTerms()
    feedback.value = 'Connections saved. Keys are kept in your OS credential store.'
  } catch { /* shared error */ } finally { key.value = ''; braveKey.value = ''; busy.value = false }
}
async function test() {
  busy.value = true
  try {
    const result = await taskCall<Capabilities>('task_test_connection', { config: active.value })
    models.value = result.models; feedback.value = result.detail
    if (result.detail.includes('Signed in')) accountNote.value = result.detail
  } catch { /* shared error */ } finally { busy.value = false }
}
async function account(action: string) {
  busy.value = true
  try {
    const result = await taskCall<{ authUrl?: string; account?: { email?: string; name?: string; accountName?: string }; signedOut?: boolean }>('task_account', { config: active.value, action })
    if (result.authUrl) { loginUrl.value = result.authUrl; await taskCall('task_open_url', { url: result.authUrl }); feedback.value = 'Finish signing in in your browser, then check account status.' }
    else if (result.signedOut) { accountNote.value = 'Signed out of ChatGPT.'; feedback.value = 'Signed out of ChatGPT.' }
    else if (result.account) {
      const who = result.account.email ?? result.account.name ?? result.account.accountName
      accountNote.value = who ? `Signed in to ChatGPT as ${who}.` : 'Not signed in to ChatGPT yet.'
      feedback.value = accountNote.value
    } else feedback.value = JSON.stringify(result, null, 2)
  } catch { /* shared error */ } finally { busy.value = false }
}
async function forget(name: string) { try { await taskCall('task_secret', { name, value: '' }); feedback.value = `${name} key removed.` } catch { /* shared error */ } }

// ---- per-companion overrides ----
const editingPet = ref<number | null>(null)
const overrideKind = ref<ProviderKind>('none')
const overrideModel = ref('')
const overrideModels = ref<string[]>([])
const overrideCount = computed(() => Object.keys(connections.value.perPet ?? {}).length)
function summarize(c: Connection) { return c.kind === 'none' ? 'No AI · built-in actions' : `${c.kind}${c.model ? ` · ${c.model}` : ''}` }
// Reuse the endpoint/executable already configured for that kind, if any.
function draftFor(kind: ProviderKind, model: string): Connection {
  const existing = [connections.value.active, ...Object.values(connections.value.perPet ?? {})].find(c => c.kind === kind)
  return {
    kind, model,
    endpoint: existing?.endpoint || (defaults[kind].endpoint ?? ''),
    executable: existing?.executable || (defaults[kind].executable ?? ''),
  }
}
function editOverride(petId: number) {
  if (editingPet.value === petId) { editingPet.value = null; return }
  const current = connections.value.perPet?.[petId] ?? effectiveConnection(petId)
  overrideKind.value = current.kind; overrideModel.value = current.model
  editingPet.value = petId
  loadOverrideModels()
}
async function loadOverrideModels() {
  overrideModels.value = []
  if (overrideKind.value === 'none') return
  try { overrideModels.value = await taskCall<string[]>('task_models', { config: draftFor(overrideKind.value, overrideModel.value) }) } catch { /* shared error */ }
}
watch(overrideKind, () => { loadOverrideModels() })
async function persistOverrides(note: string) {
  busy.value = true
  try { await taskCall('task_save_connections', { config: connections.value }); feedback.value = note } catch { /* shared error */ } finally { busy.value = false }
}
async function saveOverride(petId: number) {
  if (!connections.value.perPet) connections.value.perPet = {}
  if (overrideKind.value === 'none') delete connections.value.perPet[petId]
  else connections.value.perPet[petId] = draftFor(overrideKind.value, overrideModel.value)
  editingPet.value = null
  await persistOverrides('Companion model saved.')
}
async function clearOverride(petId: number) {
  delete connections.value.perPet?.[petId]
  await persistOverrides('Companion now uses the default connection.')
}
onMounted(async () => {
  try { detection.value = await taskCall<ProviderDetection>('task_detect_providers') } catch { /* shared error */ }
  await loadModels()
  await checkStatus()
  await loadAppleTerms()
})
</script>
<template>
  <section class="connections-panel">
    <h3>Your AI connection</h3>
    <p>Use an app installed on this computer or your own API key. Perch does not sell AI credits.</p>
    <p class="connection-summary">Default: {{ summarize(active) }}<template v-if="overrideCount"> · {{ overrideCount }} companion {{ overrideCount === 1 ? 'override' : 'overrides' }}</template></p>
    <div class="connection-sections">
      <div class="connection-section">
        <h4>Installed apps</h4>
        <p class="detection-note">Found automatically on this computer.</p>
        <label>Use an installed app
          <select v-model="installedChoice">
            <option value="">No AI · built-in actions</option>
            <option v-if="showApple" value="apple">Apple Intelligence · on this Mac</option>
            <option v-if="showCodex" value="codex">ChatGPT · Codex</option>
            <option v-if="showClaude" value="claude">Claude · included runtime</option>
            <option v-if="showOllama" value="ollama">Ollama · local models</option>
          </select>
        </label>
        <p v-if="!showApple && !showCodex && !showClaude && !showOllama" class="detection-note">Nothing detected yet. Install an app and reopen this panel.</p>
      </div>
      <div class="connection-section">
        <h4>API keys</h4>
        <p class="detection-note">Cloud models billed to your own key.</p>
        <label>Use an API key
          <select v-model="apiChoice">
            <option value="">No AI · built-in actions</option>
            <option value="openai">OpenAI{{ detection?.openaiKey ? ' · key saved' : '' }}</option>
            <option value="anthropic">Anthropic{{ detection?.anthropicKey ? ' · key saved' : '' }}</option>
            <option value="compatible">OpenAI-compatible endpoint</option>
          </select>
        </label>
      </div>
    </div>
    <template v-if="active.kind !== 'none'">
      <p v-if="providerStatus" class="provider-status" role="status">{{ providerStatus }}</p>
      <p v-if="accountNote" class="provider-status">{{ accountNote }}</p>
      <label v-if="active.kind !== 'apple'">Model<SearchSelect v-model="active.model" :options="models" :placeholder="modelPlaceholder" /></label>
      <template v-if="active.kind === 'apple'">
        <p>Runs Apple's on-device model through the built-in Foundation Models tool. Nothing is sent off this Mac, and no API key or account is needed.</p>
        <button v-if="detection && !detection.appleReady" class="text-link" @click="openAppleSettings">Open System Settings</button>
        <div class="apple-terms">
          <label class="check-label"><input v-model="appleAgreed" type="checkbox"> I accept the terms for using the on-device model</label>
          <button class="text-link" @click="appleTermsOpen = !appleTermsOpen">{{ appleTermsOpen ? 'Hide terms' : 'Read the terms' }}</button>
          <pre v-if="appleTermsOpen" class="apple-terms-text">{{ appleTerms?.text }}</pre>
          <p v-if="appleBlocked" class="detection-note">Accept these terms to save and use this connection.</p>
        </div>
      </template>
      <label v-if="['ollama', 'compatible'].includes(active.kind)">Endpoint<input v-model="active.endpoint" spellcheck="false"></label>
      <label v-if="active.kind === 'codex'">Codex executable<input v-model="active.executable" placeholder="codex or an absolute path"></label>
      <template v-if="['openai', 'anthropic', 'compatible', 'claude'].includes(active.kind)">
        <label>API key<input v-model="key" type="password" autocomplete="off" placeholder="Leave empty to keep the saved key"></label>
        <button class="text-link" @click="forget(active.kind === 'claude' ? 'anthropic' : active.kind)">Remove saved key</button>
      </template>
      <p v-if="active.kind === 'claude'">Claude subscription login is not offered. This bridge uses your Anthropic API key and requires the packaged SDK runtime.</p>
      <div v-if="active.kind === 'codex'" class="task-actions"><button :disabled="busy" @click="account('login')">Sign in with ChatGPT</button><button :disabled="busy" @click="account('status')">Check account</button><button :disabled="busy" @click="account('logout')">Sign out</button></div>
      <p v-if="loginUrl">The sign-in page opened in your browser.</p>
    </template>
    <hr>
    <h4>Companion models</h4>
    <p class="detection-note">Each companion uses the default connection above unless you give it its own.</p>
    <div v-for="pet in pets" :key="pet.id" class="per-pet-row">
      <div class="per-pet-summary">
        <strong>{{ pet.name }}</strong>
        <span>{{ summarize(effectiveConnection(pet.id)) }}</span>
        <button class="text-link" @click="editOverride(pet.id)">{{ editingPet === pet.id ? 'Close' : 'Select another model' }}</button>
        <button v-if="connections.perPet?.[pet.id]" class="text-link" @click="clearOverride(pet.id)">Use default</button>
      </div>
      <div v-if="editingPet === pet.id" class="per-pet-edit">
        <label>Provider
          <select v-model="overrideKind">
            <option value="none">No AI · built-in actions</option>
            <option v-if="showApple" value="apple">Apple Intelligence · on this Mac</option>
            <option v-if="showCodex" value="codex">ChatGPT · Codex</option>
            <option v-if="showClaude" value="claude">Claude</option>
            <option v-if="showOllama" value="ollama">Ollama · local models</option>
            <option value="openai">OpenAI · API key</option>
            <option value="anthropic">Anthropic · API key</option>
            <option value="compatible">OpenAI-compatible endpoint</option>
          </select>
        </label>
        <label v-if="!['none', 'apple'].includes(overrideKind)">Model<SearchSelect v-model="overrideModel" :options="overrideModels" placeholder="Search or type a model ID" /></label>
        <div class="task-actions"><button class="primary-button" :disabled="busy" @click="saveOverride(pet.id)">Set for {{ pet.name }}</button></div>
      </div>
    </div>
    <hr>
    <label class="check-label"><input v-model="connections.searchEnabled" type="checkbox"> Enable Brave web search</label>
    <label v-if="connections.searchEnabled">Brave Search API key<input v-model="braveKey" type="password" autocomplete="off" placeholder="Leave empty to keep the saved key"></label>
    <button v-if="connections.searchEnabled" class="text-link" @click="forget('brave')">Remove search key</button>
    <div class="task-actions"><button class="primary-button" :disabled="busy || appleBlocked" @click="save">Save connections</button><button :disabled="busy || active.kind === 'none' || appleBlocked" @click="test">{{ busy ? 'Working…' : 'Test connection' }}</button></div>
    <p role="status">{{ feedback }}</p>
  </section>
</template>
<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { Check, ChevronDown } from '@lucide/vue'
import SearchSelect from '../components/SearchSelect.vue'
import { addedProviders, connectionDefaults, connections, detection, loadDetection, loadProviderTerms, providerReady, providerTerms, taskCall } from './state'
import type { Capabilities, Connections, ProviderKind } from './types'

type Kind = Exclude<ProviderKind, 'none'>
const names: Record<Kind, string> = {
  apple: 'Apple Intelligence', codex: 'ChatGPT · Codex', claude: 'Claude Code', ollama: 'Ollama',
  anthropic: 'Anthropic', openai: 'OpenAI', compatible: 'Custom endpoint',
}
const blurbs: Record<Kind, string> = {
  apple: "Apple's on-device model. Runs entirely on this Mac, with no account or key. Basic: it answers questions from one look at your folder, and can't change files.",
  codex: 'Your ChatGPT account through the Codex app. Works as an agent you approve step by step.',
  claude: 'Your Claude Code sign-in. Development builds only.',
  ollama: 'Local models from Ollama. Nothing leaves this computer.',
  anthropic: 'Claude models, billed to your own Anthropic API key.',
  openai: 'GPT models, billed to your own OpenAI API key.',
  compatible: 'Any OpenAI-compatible service, with its own address and key.',
}
const keyed: Kind[] = ['openai', 'anthropic', 'compatible']
/** Providers whose tasks fail without a default model; agents pick their own. */
const needsModel: Kind[] = ['ollama', 'openai', 'anthropic', 'compatible']

// Offered wherever they could run, so the reason one is not ready is visible
// rather than the option silently missing. An added one always stays listed.
const onComputer = computed(() => (['apple', 'codex', 'claude', 'ollama'] as Kind[]).filter(kind => {
  if (connections.value.providers?.[kind]) return true
  if (kind === 'apple') return !!detection.value?.appleSupported
  if (kind === 'claude') return !!detection.value?.claudeCode
  return true
}))
const withKey: Kind[] = ['anthropic', 'openai', 'compatible']

function found(kind: Kind): boolean {
  const d = detection.value
  if (!d) return true
  if (kind === 'codex') return d.codexInstalled
  if (kind === 'ollama') return d.ollamaApp || d.ollamaReachable
  return true
}
function pill(kind: Kind): { text: string; tone: string } | null {
  if (connections.value.providers?.[kind]) return providerReady(kind) ? { text: 'Added', tone: 'added' } : { text: 'Accept terms', tone: 'warn' }
  return found(kind) ? null : { text: 'Not found', tone: 'muted' }
}
function status(kind: Kind): string {
  const d = detection.value
  if (!d) return ''
  switch (kind) {
    case 'apple': return d.appleDetail
    case 'codex': return d.codexInstalled ? 'Codex was found on this computer.' : 'Codex was not found on your PATH. Give its full path below.'
    case 'claude': return d.claudeCode ? 'Uses the account you signed in to with `claude` in a terminal.' : 'Claude Code sign-in is only available in development builds.'
    case 'ollama': return d.ollamaReachable ? 'Ollama is running.' : d.ollamaApp ? 'Ollama is installed but not running. Start it to list models.' : 'Ollama was not detected on this computer.'
    case 'openai': return d.openaiKey ? 'Your OpenAI key is saved in the OS credential store.' : 'Paste an OpenAI API key to list its models.'
    case 'anthropic': return d.anthropicKey ? 'Your Anthropic key is saved in the OS credential store.' : 'Paste an Anthropic API key to list its models.'
    case 'compatible': return d.compatibleKey ? 'A key for this endpoint is saved in the OS credential store.' : 'Add a key below if the endpoint needs one.'
  }
}
const keySaved = (kind: Kind) => !!detection.value && (
  (kind === 'openai' && detection.value.openaiKey) || (kind === 'anthropic' && detection.value.anthropicKey) || (kind === 'compatible' && detection.value.compatibleKey))

// ---- the one provider being set up or edited ----
const open = ref<Kind | null>(null)
const draft = reactive({ model: '', endpoint: '', executable: '', key: '', agreed: false, showTerms: false })
const models = ref<string[]>([])
const listing = ref(false)
const busy = ref(false)
const feedback = ref('')
const account = ref('')
const confirmRemove = ref(false)

const added = computed(() => !!open.value && !!connections.value.providers?.[open.value])
/** Terms are asked for only when this build's version has not been accepted yet. */
const termsDue = computed(() => !!open.value && !!providerTerms.value[open.value] && !providerReady(open.value))
const missing = computed(() => {
  const kind = open.value
  if (!kind) return ''
  if (needsModel.includes(kind) && !draft.model.trim()) return 'Choose a default model.'
  if (termsDue.value && !draft.agreed) return 'Accept the terms to continue.'
  return ''
})
const acceptedOn = computed(() => {
  const at = open.value ? connections.value.providers?.[open.value]?.acceptedAt : 0
  return at ? new Date(at * 1000).toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: 'numeric' }) : ''
})

function draftConnection() {
  return { kind: open.value!, model: draft.model, endpoint: draft.endpoint, executable: draft.executable }
}
function toggle(kind: Kind) {
  if (open.value === kind) { open.value = null; return }
  const saved = connections.value.providers?.[kind]
  const defaults = connectionDefaults[kind]
  Object.assign(draft, {
    model: saved?.model ?? defaults.model ?? '', endpoint: saved?.endpoint || (defaults.endpoint ?? ''),
    executable: saved?.executable || (defaults.executable ?? ''), key: '', agreed: false, showTerms: false,
  })
  feedback.value = ''; account.value = ''; confirmRemove.value = false; models.value = []
  open.value = kind
  void listModels()
  if (kind === 'codex') void codex('status', true)
}
async function listModels() {
  const kind = open.value
  models.value = []
  if (!kind || kind === 'apple' || kind === 'claude') return
  listing.value = true
  // A failed listing (no key yet, service stopped) still leaves the model typed by hand.
  try { models.value = await taskCall<string[]>('task_models', { config: draftConnection() }) } catch { /* shared error */ } finally { listing.value = false }
}
async function saveKey() {
  const kind = open.value
  if (!kind || !draft.key) return
  await taskCall('task_secret', { name: kind, value: draft.key })
  draft.key = ''
  await loadDetection()
}
async function add() {
  const kind = open.value
  if (!kind || missing.value) return
  busy.value = true
  const wasDefault = connections.value.active.kind
  try {
    await saveKey()
    connections.value = await taskCall<Connections>('task_add_provider', {
      kind, setup: { model: draft.model.trim(), endpoint: draft.endpoint.trim(), executable: draft.executable.trim(), termsVersion: 0, acceptedAt: 0 }, accept: draft.agreed,
    })
    const nowDefault = wasDefault === 'none' && connections.value.active.kind === kind
    feedback.value = added.value && !nowDefault ? 'Saved.' : nowDefault ? `Added. ${names[kind]} is now the default for your pets.` : `Added. Pick ${names[kind]} for any pet in the Assistant.`
    draft.agreed = false
  } catch { /* shared error */ } finally { busy.value = false }
}
async function test() {
  busy.value = true
  try {
    await saveKey()
    const result = await taskCall<Capabilities>('task_test_connection', { config: draftConnection() })
    if (result.models.length) models.value = result.models
    feedback.value = result.detail
  } catch { /* shared error */ } finally { busy.value = false }
}
async function remove() {
  const kind = open.value
  if (!kind) return
  if (!confirmRemove.value) { confirmRemove.value = true; return }
  busy.value = true
  try {
    connections.value = await taskCall<Connections>('task_remove_provider', { kind })
    if (keyed.includes(kind)) { await taskCall('task_secret', { name: kind, value: '' }); await loadDetection() }
    open.value = null
  } catch { /* shared error */ } finally { busy.value = false; confirmRemove.value = false }
}
type Account = { authUrl?: string; account?: { email?: string; name?: string; accountName?: string }; signedOut?: boolean }
async function codex(action: 'login' | 'status' | 'logout', quiet = false) {
  if (quiet && !detection.value?.codexInstalled) return
  busy.value = true
  try {
    const result = await taskCall<Account>('task_account', { config: draftConnection(), action })
    if (result.authUrl) { await taskCall('task_open_url', { url: result.authUrl }); account.value = 'Finish signing in in your browser, then check your account.' }
    else if (result.signedOut) account.value = 'Signed out of ChatGPT.'
    else {
      const who = result.account?.email ?? result.account?.name ?? result.account?.accountName
      account.value = who ? `Signed in to ChatGPT as ${who}.` : 'Not signed in to ChatGPT yet.'
    }
  } catch { /* shared error */ } finally { busy.value = false }
}
async function openAppleSettings() {
  try { await taskCall('task_apple_open_settings') } catch { /* shared error */ }
}

// ---- web search, independent of the AI provider ----
const braveKey = ref('')
const searchNote = ref('')
async function saveSearch(enabled: boolean) {
  try {
    if (braveKey.value) { await taskCall('task_secret', { name: 'brave', value: braveKey.value }); braveKey.value = ''; await loadDetection() }
    connections.value = await taskCall<Connections>('task_save_connections', { config: { ...connections.value, searchEnabled: enabled } })
    searchNote.value = enabled ? 'Web search is on.' : 'Web search is off.'
  } catch { /* shared error */ }
}
async function saveThinking(enabled: boolean) {
  try {
    connections.value = await taskCall<Connections>('task_save_connections', { config: { ...connections.value, ollamaThinking: enabled } })
  } catch { /* shared error */ }
}
async function forgetBrave() {
  try { await taskCall('task_secret', { name: 'brave', value: '' }); await loadDetection(); searchNote.value = 'Search key removed.' } catch { /* shared error */ }
}

onMounted(() => { void loadDetection(); void loadProviderTerms() })
</script>

<template>
  <div class="providers-page">
    <section class="page-heading">
      <div>
        <h1>AI providers</h1>
        <p>Set each one up once. Then choose between them for any pet in the Assistant.</p>
      </div>
    </section>
    <p class="providers-summary">{{ addedProviders.length ? `${addedProviders.length} added` : 'Nothing added yet' }} · Ledgeling does not sell AI credits; you use your own apps and keys.</p>

    <template v-for="group in [{ title: 'On this computer', kinds: onComputer }, { title: 'With your API key', kinds: withKey }]" :key="group.title">
      <h2 class="group-title">{{ group.title }}</h2>
      <section class="settings-group">
        <div v-for="kind in group.kinds" :key="kind" class="provider-item" :class="{ open: open === kind }">
          <div class="setting-row">
            <div><h3>{{ names[kind] }}</h3><p>{{ blurbs[kind] }}</p></div>
            <div class="row-controls">
              <span v-if="pill(kind)" class="provider-pill" :data-tone="pill(kind)!.tone"><Check v-if="pill(kind)!.tone === 'added'" :size="12" />{{ pill(kind)!.text }}</span>
              <button type="button" :aria-expanded="open === kind" @click="toggle(kind)">
                {{ connections.providers?.[kind] ? 'Edit' : 'Set up' }}<ChevronDown :size="14" class="chevron" />
              </button>
            </div>
          </div>

          <div v-if="open === kind" class="provider-setup">
            <p v-if="status(kind)" class="provider-status" role="status">{{ status(kind) }}</p>
            <button v-if="kind === 'apple' && detection && !detection.appleReady" type="button" class="text-link" @click="openAppleSettings">Open System Settings</button>

            <label v-if="kind === 'ollama' || kind === 'compatible'">Address<input v-model="draft.endpoint" spellcheck="false" @change="listModels"></label>
            <label v-if="kind === 'codex'">Codex app<input v-model="draft.executable" spellcheck="false" placeholder="codex, or its full path"></label>
            <template v-if="keyed.includes(kind)">
              <label>API key<input v-model="draft.key" type="password" autocomplete="off" :placeholder="keySaved(kind) ? 'Saved. Paste a new one to replace it' : 'Paste your key'" @change="saveKey().then(listModels)"></label>
            </template>
            <label v-if="kind !== 'apple' && kind !== 'claude'">Default model
              <SearchSelect v-model="draft.model" :options="models" :placeholder="listing ? 'Listing models…' : kind === 'codex' ? 'Codex default' : 'Search or type a model ID'" />
            </label>
            <div v-if="kind === 'codex'" class="provider-account">
              <span v-if="account">{{ account }}</span>
              <button type="button" class="text-link" :disabled="busy" @click="codex('login')">Sign in with ChatGPT</button>
              <button type="button" class="text-link" :disabled="busy" @click="codex('status')">Check account</button>
              <button type="button" class="text-link" :disabled="busy" @click="codex('logout')">Sign out</button>
            </div>

            <div v-if="providerTerms[kind]" class="provider-terms">
              <template v-if="termsDue">
                <h4>Terms</h4>
                <pre class="provider-terms-text">{{ providerTerms[kind]!.text }}</pre>
                <label class="check-label"><input v-model="draft.agreed" type="checkbox"> I accept these terms</label>
              </template>
              <p v-else class="terms-accepted">
                <Check :size="13" /> Terms accepted{{ acceptedOn ? ` on ${acceptedOn}` : '' }}.
                <button type="button" class="text-link" @click="draft.showTerms = !draft.showTerms">{{ draft.showTerms ? 'Hide' : 'Read them' }}</button>
              </p>
              <pre v-if="!termsDue && draft.showTerms" class="provider-terms-text">{{ providerTerms[kind]!.text }}</pre>
            </div>

            <div class="provider-actions">
              <button type="button" class="primary-button" :disabled="busy || !!missing" @click="add">{{ added ? 'Save changes' : 'Add to Ledgeling' }}</button>
              <button v-if="added" type="button" :disabled="busy" @click="test">{{ busy ? 'Working…' : 'Test' }}</button>
              <button v-if="added" type="button" class="text-link danger" :disabled="busy" @click="remove">{{ confirmRemove ? `Remove ${names[kind]}?` : 'Remove' }}</button>
              <span v-if="missing" class="provider-missing">{{ missing }}</span>
            </div>
            <p v-if="feedback" class="provider-feedback" role="status">{{ feedback }}</p>
          </div>
        </div>
      </section>
    </template>

    <h2 class="group-title">Web search</h2>
    <section v-if="connections.providers.ollama" class="settings-group">
      <div class="setting-row">
        <div><h3>Let local models think</h3><p>Reasoning models such as Qwen3 can think before they answer. Answers may be better, but a small model can take a minute or more and look stuck. Off by default.</p></div>
        <button class="toggle" :class="{ enabled: connections.ollamaThinking }" role="switch" :aria-checked="connections.ollamaThinking" aria-label="Let local models think"
                @click="saveThinking(!connections.ollamaThinking)"><span /></button>
      </div>
    </section>
    <section class="settings-group">
      <div class="setting-row">
        <div><h3>Brave Search</h3><p>Lets any provider look things up when you turn on Web for a request. Uses your own Brave Search API key.</p></div>
        <button class="toggle" :class="{ enabled: connections.searchEnabled }" role="switch" :aria-checked="connections.searchEnabled" aria-label="Web search"
                @click="saveSearch(!connections.searchEnabled)"><span /></button>
      </div>
      <div v-if="connections.searchEnabled" class="provider-setup">
        <label>Brave Search API key<input v-model="braveKey" type="password" autocomplete="off" :placeholder="detection?.braveKey ? 'Saved. Paste a new one to replace it' : 'Paste your key'" @change="saveSearch(true)"></label>
        <button v-if="detection?.braveKey" type="button" class="text-link" @click="forgetBrave">Remove search key</button>
        <p v-if="searchNote" class="provider-feedback" role="status">{{ searchNote }}</p>
      </div>
    </section>
  </div>
</template>

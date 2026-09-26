<script setup lang="ts">
import { computed } from 'vue'
import { Flag, Plus, Trash2, X, Zap } from '@lucide/vue'
import BlockSelect from './BlockSelect.vue'
import {
  actions, categories, clipInfo, clipsFor, equalityOps, events, nextKey, numberOps, renameState, signals, stateName,
  type Draft, type StateDraft, type Transition,
} from './model'

const props = defineProps<{ draft: Draft }>()

const stateOptions = computed(() => props.draft.states.map(s => ({ value: s.name, label: s.name.replace(/_/g, ' ') })))
const clipOptions = computed(() => clipsFor(props.draft.movement).map(c => ({
  value: c.name,
  label: props.draft.clips[c.name] ? c.label : `${c.label} (not drawn yet)`,
})))
const signalOptions = signals.map(s => ({ value: s.value, label: s.label }))
const categoryOptions = categories.map(c => ({ value: c, label: c === 'comms' ? 'chat & email' : c }))
const boolOptions = [{ value: 'true', label: 'yes' }, { value: 'false', label: 'no' }]

function signal(name: string) { return signals.find(s => s.value === name)! }
function opsFor(name: string) { return signal(name).kind === 'number' ? numberOps : equalityOps }
function setSignal(condition: { signal: string; op: string; value: string }, name: string) {
  const info = signal(name)
  condition.signal = name
  if (!opsFor(name).some(o => o.value === condition.op)) condition.op = '=='
  condition.value = info.example
}

/** How likely each next step is, among the ones that could apply at the same time. */
function chance(state: StateDraft, t: Transition) {
  const total = state.transitions.filter(o => !o.when.length || o === t).reduce((sum, o) => sum + Math.max(0, o.weight), 0)
  return total ? Math.round((Math.max(0, t.weight) / total) * 100) : 0
}
function incoming(state: StateDraft) {
  const from = props.draft.states.filter(s => s !== state && s.transitions.some(t => t.to === state.name)).map(s => s.name.replace(/_/g, ' '))
  const reactions = props.draft.reactions.filter(r => r.goto === state.name).map(r => events.find(e => e.value === r.on)?.label ?? r.on)
  return { from, reactions }
}

function rename(state: StateDraft, text: string) {
  let next = stateName(text)
  if (props.draft.states.some(s => s !== state && s.name === next)) next = `${next}_2`
  renameState(props.draft, state.name, next)
}
function addState() {
  let n = props.draft.states.length + 1
  while (props.draft.states.some(s => s.name === `behaviour_${n}`)) n++
  props.draft.states.push({ key: nextKey(), name: `behaviour_${n}`, clip: 'idle', action: '', duration: [3, 8], transitions: [{ to: props.draft.initial, weight: 1, when: [] }] })
}
function removeState(state: StateDraft) {
  if (state.name === props.draft.initial) return
  props.draft.states = props.draft.states.filter(s => s !== state)
  for (const s of props.draft.states) s.transitions = s.transitions.filter(t => t.to !== state.name)
  props.draft.reactions = props.draft.reactions.filter(r => r.goto !== state.name)
}
function addReaction() {
  const used = new Set(props.draft.reactions.map(r => r.on))
  const on = events.find(e => !used.has(e.value))?.value ?? 'clicked'
  props.draft.reactions.push({ key: nextKey(), on, goto: props.draft.initial, cooldown: null })
}
function numberOrNull(event: Event) {
  const value = (event.target as HTMLInputElement).valueAsNumber
  return Number.isFinite(value) && value > 0 ? value : null
}
</script>

<template>
  <div class="behavior-editor">
    <section class="block-section">
      <header class="block-section-head">
        <h2><Zap :size="16" /> When something happens</h2>
        <p>Reactions interrupt whatever your pet is doing.</p>
      </header>
      <ul class="block-list">
        <li v-for="(reaction, i) in draft.reactions" :key="reaction.key" class="block hat-block">
          <span class="block-word">When</span>
          <BlockSelect v-model="reaction.on" label="Event" :options="events" />
          <span class="block-word">go to</span>
          <BlockSelect v-model="reaction.goto" label="Behaviour" :options="stateOptions" />
          <span class="block-word soft">at most every</span>
          <input class="block-number" type="number" min="0" step="1" :value="reaction.cooldown ?? ''" placeholder="–" aria-label="Cooldown in seconds" @change="reaction.cooldown = numberOrNull($event)">
          <span class="block-word soft">sec</span>
          <button type="button" class="block-remove" aria-label="Remove reaction" @click="draft.reactions.splice(i, 1)"><X :size="14" /></button>
        </li>
      </ul>
      <button type="button" class="add-block" @click="addReaction"><Plus :size="14" /> Add a reaction</button>
    </section>

    <section class="block-section">
      <header class="block-section-head">
        <h2><Flag :size="16" /> Behaviours</h2>
        <p>Your pet does one of these at a time, then picks what comes next.</p>
      </header>
      <article v-for="state in draft.states" :key="state.key" class="state-block" :class="{ initial: state.name === draft.initial }">
        <header class="state-head">
          <input class="state-name" :value="state.name.replace(/_/g, ' ')" aria-label="Behaviour name" spellcheck="false"
                 @change="rename(state, ($event.target as HTMLInputElement).value)">
          <span v-if="state.name === draft.initial" class="start-badge"><Flag :size="12" /> Starts here</span>
          <button v-else type="button" class="text-link" @click="draft.initial = state.name">Start here</button>
          <button v-if="state.name !== draft.initial" type="button" class="block-remove" aria-label="Delete behaviour" @click="removeState(state)"><Trash2 :size="14" /></button>
        </header>

        <div class="block do-block">
          <span class="block-word">Play</span>
          <BlockSelect v-model="state.clip" label="Animation" :options="clipOptions" />
          <span class="block-word">and</span>
          <BlockSelect v-model="state.action" label="Action" :options="actions" />
          <template v-if="state.duration">
            <span class="block-word">for</span>
            <input v-model.number="state.duration[0]" class="block-number" type="number" min="0" step="1" aria-label="Shortest time in seconds">
            <span class="block-word">to</span>
            <input v-model.number="state.duration[1]" class="block-number" type="number" min="0" step="1" aria-label="Longest time in seconds">
            <span class="block-word">sec</span>
            <button type="button" class="block-remove" title="No time limit" aria-label="Remove time limit" @click="state.duration = null"><X :size="13" /></button>
          </template>
          <template v-else>
            <span class="block-word soft">{{ state.action ? 'until it gets there' : 'for a while' }}</span>
            <button type="button" class="text-link" @click="state.duration = [3, 8]">set a time</button>
          </template>
        </div>
        <p v-if="!draft.clips[state.clip]" class="block-note">{{ clipInfo(state.clip)?.label }} isn't drawn yet, so this plays {{ clipInfo(clipInfo(state.clip)?.fallback ?? 'idle')?.label }}.</p>

        <div class="then-block">
          <span class="then-label">Then</span>
          <ul class="block-list">
            <li v-for="(t, ti) in state.transitions" :key="ti" class="block next-block">
              <span class="block-word">go to</span>
              <BlockSelect v-model="t.to" label="Next behaviour" :options="stateOptions" />
              <span class="block-word soft">weight</span>
              <input v-model.number="t.weight" class="block-number" type="number" min="0" step="1" aria-label="Weight">
              <span class="chance" :title="'How often this is picked, when it can be'">{{ chance(state, t) }}%</span>
              <button type="button" class="block-remove" aria-label="Remove next step" @click="state.transitions.splice(ti, 1)"><X :size="14" /></button>
              <div v-for="(c, ci) in t.when" :key="ci" class="condition">
                <span class="block-word">{{ ci === 0 ? 'only if' : 'and' }}</span>
                <BlockSelect :model-value="c.signal" label="Signal" :options="signalOptions" @update:model-value="setSignal(c, $event)" />
                <BlockSelect v-model="c.op" label="Comparison" :options="opsFor(c.signal)" />
                <BlockSelect v-if="signal(c.signal).kind === 'category'" v-model="c.value" label="Kind of app" :options="categoryOptions" />
                <BlockSelect v-else-if="signal(c.signal).kind === 'bool'" v-model="c.value" label="Yes or no" :options="boolOptions" />
                <template v-else>
                  <input v-model="c.value" class="block-number" inputmode="decimal" aria-label="Value">
                  <span v-if="signal(c.signal).unit" class="block-word soft">{{ signal(c.signal).unit }}</span>
                </template>
                <button type="button" class="block-remove" aria-label="Remove condition" @click="t.when.splice(ci, 1)"><X :size="13" /></button>
              </div>
              <button type="button" class="text-link add-condition" @click="t.when.push({ signal: 'idle_seconds', op: '>', value: '240' })"><Plus :size="12" /> only if…</button>
            </li>
          </ul>
          <button type="button" class="add-block small" @click="state.transitions.push({ to: draft.initial, weight: 1, when: [] })"><Plus :size="13" /> Add a next step</button>
        </div>

        <footer class="state-foot">
          <template v-if="incoming(state).from.length || incoming(state).reactions.length">
            Reached from {{ [...incoming(state).from, ...incoming(state).reactions.map(r => `when ${r}`)].join(', ') }}.
          </template>
          <template v-else-if="state.name !== draft.initial"><span class="warn">Nothing leads here yet.</span></template>
          <template v-else>Your pet starts here.</template>
        </footer>
      </article>
      <button type="button" class="add-block" @click="addState"><Plus :size="14" /> Add a behaviour</button>
    </section>
  </div>
</template>

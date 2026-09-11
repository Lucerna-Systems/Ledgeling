<script setup lang="ts">
import { ShieldCheck, Check, X, Power, Heart } from '@lucide/vue'
import { call, saveSettings, settings, native } from '../state'

const collected = [
  'How many seconds since you last touched the keyboard or mouse',
  'Which kind of application is in front — code, browser, chat, media, creative, game, other',
  'The position and size of your open windows, and which is in front',
  'Where your mouse pointer is, so the pet can react to it',
  'How long your current unbroken stretch of work has been',
]
const never = [
  'Window titles — they hold email subjects, document names and URLs',
  'What you type; only that input happened, never what it was',
  'Screen contents. Perch takes no screenshots and asks for no recording permission',
  'The clipboard, browser history or open tabs through passive observation',
  'Background uploads or telemetry from companion awareness',
]
</script>

<template>
  <section class="page-heading">
    <div>
      <div class="eyebrow">LESS NOISE. MORE TRUST.</div>
      <h1>Privacy & preferences</h1>
      <p>Your data, your connections, your choices.</p>
    </div>
  </section>

  <section class="privacy-card">
    <ShieldCheck :size="34" :stroke-width="1.2" />
    <h2>Your companion stays private. Tasks are opt-in.</h2>
    <p>
      It checks how long since you last touched the keyboard, and which <em>kind</em> of application is in
      front — code editor, browser, chat, and so on. It never reads window titles, keystrokes or your screen.
      Tasks can read files you select. Local Ollama processes them on your computer; cloud providers receive
      selected task content only when you allow it. Web tasks contact search services and public pages.
      There is no Perch account or telemetry, and no automatic local-to-cloud fallback.
    </p>
  </section>

  <div class="privacy-columns">
    <section>
      <h3>What is collected</h3>
      <ul class="privacy-list">
        <li v-for="item in collected" :key="item"><Check :size="15" /> {{ item }}</li>
      </ul>
      <p class="muted">Awareness stays in memory. Settings, task history, selected paths, results and operation journals are stored locally. API keys are stored separately in the OS credential store.</p>
    </section>
    <section>
      <h3>What is never collected</h3>
      <ul class="privacy-list never">
        <li v-for="item in never" :key="item"><X :size="15" /> {{ item }}</li>
      </ul>
      <p class="muted">Privacy tests prohibit passive window-title, screen-capture and key-state APIs. Network code is confined to explicit task connections; the pet engine has no network access.</p>
    </section>
  </div>

  <section class="controls-card">
    <div class="desktop-details">
      <div><h3>Turn all awareness off</h3><p>The pet keeps living on your windows; it just stops noticing your day.</p></div>
      <button class="toggle" :class="{ enabled: settings?.awareness.enabled }" role="switch"
              :aria-checked="!!settings?.awareness.enabled" aria-label="Awareness"
              @click="settings && saveSettings({ ...settings, awareness: { ...settings.awareness, enabled: !settings.awareness.enabled } })"><span /></button>
    </div>
    <div class="desktop-details">
      <div><h3>Sounds</h3><p>Off by default. Only plays clips a pack supplies, and only on its own states.</p></div>
      <button class="toggle" :class="{ enabled: settings?.sounds }" role="switch" :aria-checked="!!settings?.sounds" aria-label="Sounds"
              @click="settings && saveSettings({ ...settings, sounds: !settings.sounds })"><span /></button>
    </div>
    <div class="desktop-details">
      <div><h3>Open Perch at login</h3><p>Off by default. You are asked once, here, and never again.</p></div>
      <button class="toggle" :class="{ enabled: settings?.launchAtLogin }" role="switch" :aria-checked="!!settings?.launchAtLogin" aria-label="Launch at login"
              @click="settings && saveSettings({ ...settings, launchAtLogin: !settings.launchAtLogin })"><span /></button>
    </div>
  </section>

  <section class="about-note">
    <Heart :size="20" :stroke-width="1.4" />
    <div>
      <h3>A small thing, on purpose</h3>
      <p>Your pet remains a companion even without AI. Advanced task mode delegates to an agent runtime with its own sandbox and per-action approvals; commands do not have universal undo.</p>
    </div>
  </section>
  <button class="quit-button" :disabled="!native" @click="call('quit_app')"><Power :size="15" /> Quit Perch</button>
</template>

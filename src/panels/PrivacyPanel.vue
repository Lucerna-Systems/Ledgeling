<script setup lang="ts">
import { ShieldCheck, Check, X, Power, Heart, Palette } from '@lucide/vue'
import { call, saveSettings, settings, native } from '../state'
import { ACCENT_PRESETS, presetFor } from '../theme'
import type { ThemeMode } from '../types'

const themeChoices: { value: ThemeMode; label: string }[] = [
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'system', label: 'Auto' },
]
function pickTheme(value: ThemeMode) {
  if (settings.value) void saveSettings({ ...settings.value, theme: value })
}
function pickAccent(accent: string) {
  if (settings.value) void saveSettings({ ...settings.value, accent })
}

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
    <div class="section-label"><Palette :size="15" /> Appearance</div>
    <div class="desktop-details">
      <div><h3>Theme</h3><p>Auto follows your system appearance and switches when it does.</p></div>
      <div class="segmented" role="radiogroup" aria-label="Theme">
        <button v-for="choice in themeChoices" :key="choice.value" type="button" role="radio"
                :aria-checked="settings?.theme === choice.value" @click="pickTheme(choice.value)">
          {{ choice.label }}
        </button>
      </div>
    </div>
    <div class="desktop-details">
      <div><h3>Accent colour</h3><p>Tints buttons, highlights and the pet's favourite spots.</p></div>
      <div class="swatches" role="radiogroup" aria-label="Accent colour">
        <button v-for="preset in ACCENT_PRESETS" :key="preset.id" type="button" class="swatch" role="radio"
                :style="{ background: preset.base }" :title="preset.label" :aria-label="preset.label"
                :aria-checked="settings?.accent?.toLowerCase() === preset.base" @click="pickAccent(preset.base)" />
        <span class="swatch custom" role="radio" title="Custom" aria-label="Custom colour"
              :aria-checked="!!settings?.accent && !presetFor(settings.accent)">
          <input type="color" :value="settings?.accent ?? '#31725b'" @change="pickAccent(($event.target as HTMLInputElement).value)" />
        </span>
      </div>
    </div>
  </section>

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

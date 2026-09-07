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
  'The clipboard, your files, your browser history or your open tabs',
  'Anything at all sent over a network. There is no account and no server',
]
</script>

<template>
  <section class="page-heading">
    <div>
      <div class="eyebrow">LESS NOISE. MORE TRUST.</div>
      <h1>Exactly what<br>Perch looks at<span>.</span></h1>
      <p>Written out in full, because a companion that watches you should say so plainly.</p>
    </div>
  </section>

  <section class="privacy-card">
    <ShieldCheck :size="34" :stroke-width="1.2" />
    <h2>Perch runs entirely on your computer.</h2>
    <p>
      It checks how long since you last touched the keyboard, and which <em>kind</em> of application is in
      front — code editor, browser, chat, and so on. It does not read window titles, file names, what you
      type, or what is on your screen. Nothing is sent anywhere. There is no account, no server, and no
      telemetry. You can turn all of this off and keep the pet.
    </p>
  </section>

  <div class="privacy-columns">
    <section>
      <h3>What is collected</h3>
      <ul class="privacy-list">
        <li v-for="item in collected" :key="item"><Check :size="15" /> {{ item }}</li>
      </ul>
      <p class="muted">All of it lives in memory only. The one thing written to disk is your settings file, plus the position each pet was last standing in.</p>
    </section>
    <section>
      <h3>What is never collected</h3>
      <ul class="privacy-list never">
        <li v-for="item in never" :key="item"><X :size="15" /> {{ item }}</li>
      </ul>
      <p class="muted">Perch's own tests assert this: the build fails if any window-title, screen-capture, key-state or network call appears in the source.</p>
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
      <p>No accounts, no streaks, no reports. Just an animal that lives on the edges of your windows and is a bit more settled when you take a break.</p>
    </div>
  </section>
  <button class="quit-button" :disabled="!native" @click="call('quit_app')"><Power :size="15" /> Quit Perch</button>
</template>

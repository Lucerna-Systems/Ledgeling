import { createApp } from 'vue'
import App from './App.vue'
import './style.css'
import { initTheme } from './theme'
// Applied before mount so the first painted frame already carries the stored
// theme and accent.
initTheme()
createApp(App).mount('#app')
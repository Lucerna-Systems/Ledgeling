import { createApp } from 'vue'
import CodeApp from './CodeApp.vue'
import '../style.css'
import './code.css'
import { initTheme } from '../theme'
// Same theme and accent as the rest of Ledgeling, painted before the first frame.
initTheme()
createApp(CodeApp)
  .directive('focus', { mounted: (el: HTMLElement) => el.focus() })
  .mount('#app')

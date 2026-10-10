import { createApp } from 'vue'
import TerminalApp from './TerminalApp.vue'
import '../style.css'
import './base.css'
import './terminal.css'
import './terminalState'
import { initTheme } from '../theme'
// Same theme and accent as the rest of Ledgeling, painted before the first frame.
initTheme()
createApp(TerminalApp).mount('#app')

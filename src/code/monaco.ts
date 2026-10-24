import * as monaco from 'monaco-editor'
import EditorWorker from 'monaco-editor/editor/editor.worker?worker'
import JsonWorker from 'monaco-editor/language/json/json.worker?worker'
import CssWorker from 'monaco-editor/language/css/css.worker?worker'
import HtmlWorker from 'monaco-editor/language/html/html.worker?worker'
import TsWorker from 'monaco-editor/language/typescript/ts.worker?worker'
import { watch } from 'vue'
import { palette, trackPalette, type Palette } from '../terminal/theme'

// Only this module pulls Monaco in, and only the Editor and Changes tabs import it.
self.MonacoEnvironment = {
  getWorker(_id: string, label: string) {
    if (label === 'json') return new JsonWorker()
    if (label === 'css' || label === 'scss' || label === 'less') return new CssWorker()
    if (label === 'html' || label === 'handlebars' || label === 'razor') return new HtmlWorker()
    if (label === 'typescript' || label === 'javascript') return new TsWorker()
    return new EditorWorker()
  },
}

function defineTheme(p: Palette) {
  monaco.editor.defineTheme('ledgeling', {
    base: p.dark ? 'vs-dark' : 'vs',
    inherit: true,
    rules: [],
    colors: {
      'editor.background': p.background,
      'editor.foreground': p.ink,
      'editorLineNumber.foreground': p.muted + '99',
      'editorLineNumber.activeForeground': p.ink,
      'editor.lineHighlightBackground': p.surface,
      'editor.lineHighlightBorder': p.surface,
      'editorCursor.foreground': p.accent,
      'editor.selectionBackground': p.accent + '40',
      'editor.inactiveSelectionBackground': p.accent + '22',
      'editorIndentGuide.background1': p.line,
      'editorWidget.background': p.background,
      'editorWidget.border': p.line,
      'editorGutter.background': p.background,
      'scrollbarSlider.background': p.muted + '33',
      'scrollbarSlider.hoverBackground': p.muted + '55',
      'diffEditor.insertedTextBackground': p.live + '26',
      'diffEditor.removedTextBackground': p.danger + '26',
      'diffEditor.insertedLineBackground': p.live + '14',
      'diffEditor.removedLineBackground': p.danger + '14',
      'focusBorder': p.accent,
    },
  })
  monaco.editor.setTheme('ledgeling')
}

let ready = false
export function setupMonaco() {
  if (ready) return monaco
  ready = true
  trackPalette()
  if (palette.value) defineTheme(palette.value)
  watch(palette, p => { if (p) defineTheme(p) })
  return monaco
}

const byExtension: Record<string, string> = {
  ts: 'typescript', tsx: 'typescript', mts: 'typescript', cts: 'typescript', js: 'javascript', jsx: 'javascript', mjs: 'javascript', cjs: 'javascript',
  json: 'json', jsonc: 'json', css: 'css', scss: 'scss', less: 'less', html: 'html', htm: 'html', vue: 'html', svelte: 'html',
  md: 'markdown', markdown: 'markdown', rs: 'rust', py: 'python', go: 'go', rb: 'ruby', java: 'java', kt: 'kotlin', swift: 'swift',
  c: 'c', h: 'c', cpp: 'cpp', cc: 'cpp', hpp: 'cpp', cs: 'csharp', php: 'php', sh: 'shell', bash: 'shell', zsh: 'shell',
  yml: 'yaml', yaml: 'yaml', toml: 'ini', ini: 'ini', sql: 'sql', xml: 'xml', svg: 'xml', dockerfile: 'dockerfile', ps1: 'powershell',
  graphql: 'graphql', lua: 'lua', dart: 'dart', r: 'r', scala: 'scala', m: 'objective-c',
}

export function languageFor(path: string) {
  const name = path.split('/').at(-1)?.toLowerCase() ?? ''
  if (name === 'dockerfile') return 'dockerfile'
  if (name === 'makefile') return 'makefile'
  return byExtension[name.split('.').at(-1) ?? ''] ?? 'plaintext'
}

export { monaco }

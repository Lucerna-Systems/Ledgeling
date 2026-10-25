// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import ProvidersPanel from './ProvidersPanel.vue'
import { connections, detection } from './state'
import type { Connections, ProviderDetection } from './types'

const api = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: api.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

const found: ProviderDetection = {
  codexInstalled: false, claudeCode: false, ollamaApp: true, ollamaReachable: true, openaiKey: false, anthropicKey: false,
  compatibleKey: false, braveKey: false, appleSupported: true, appleReady: true, appleDetail: 'Apple Intelligence is ready. Runs entirely on this Mac.',
}
const empty = (): Connections => ({ active: { kind: 'none', model: '', endpoint: '', executable: '' }, searchEnabled: false, ollamaThinking: false, perPet: {}, providers: {} })

beforeEach(() => {
  connections.value = empty()
  detection.value = null
  api.invoke.mockReset().mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'task_detect_providers') return found
    if (command === 'task_provider_terms') return { apple: { version: 1, text: 'Output may be inaccurate.' }, anthropic: { version: 1, text: 'Content leaves this computer.' } }
    if (command === 'task_models') return (args!.config as { kind: string }).kind === 'anthropic' ? ['claude-sonnet-5'] : []
    if (command === 'task_add_provider') {
      const kind = args!.kind as 'apple' | 'anthropic'
      const setup = { ...(args!.setup as object), termsVersion: 1, acceptedAt: 1_790_000_000 }
      return { ...empty(), active: { kind, model: kind === 'apple' ? 'system' : 'claude-sonnet-5', endpoint: '', executable: '' }, providers: { [kind]: setup } }
    }
    return null
  })
})

function row(wrapper: ReturnType<typeof mount>, name: string) {
  return wrapper.findAll('.provider-item').find(item => item.get('h3').text() === name)!
}

it('Apple Intelligence is added once its terms are accepted, and becomes the default', async () => {
  const wrapper = mount(ProvidersPanel)
  await flushPromises()
  await row(wrapper, 'Apple Intelligence').get('.row-controls button').trigger('click')
  const apple = () => row(wrapper, 'Apple Intelligence')
  expect(apple().get('.provider-terms-text').text()).toContain('inaccurate')
  expect(apple().get('.primary-button').attributes('disabled')).toBeDefined()
  expect(apple().text()).toContain('Accept the terms to continue.')
  await apple().get('.provider-terms input[type=checkbox]').setValue(true)
  await apple().get('.primary-button').trigger('click')
  await flushPromises()
  const [, args] = api.invoke.mock.calls.find(([command]) => command === 'task_add_provider')!
  expect(args).toMatchObject({ kind: 'apple', accept: true })
  expect(connections.value.active.kind).toBe('apple')
  expect(apple().get('.provider-pill').text()).toBe('Added')
  expect(apple().text()).toContain('now the default')
  // Accepted terms are not asked for again when the provider is edited.
  expect(apple().find('.provider-terms input[type=checkbox]').exists()).toBe(false)
  expect(apple().text()).toContain('Terms accepted')
  wrapper.unmount()
})

it('an API provider saves its key first and needs a default model', async () => {
  const wrapper = mount(ProvidersPanel)
  await flushPromises()
  await row(wrapper, 'Anthropic').get('.row-controls button').trigger('click')
  const anthropic = () => row(wrapper, 'Anthropic')
  await anthropic().get('input[type=password]').setValue('sk-ant-test')
  await anthropic().get('.provider-terms input[type=checkbox]').setValue(true)
  expect(anthropic().text()).toContain('Choose a default model.')
  const model = anthropic().get('.search-select input')
  await model.setValue('claude-sonnet-5')
  await model.trigger('keydown', { key: 'Enter' })
  await anthropic().get('.primary-button').trigger('click')
  await flushPromises()
  const commands = api.invoke.mock.calls.map(([command]) => command)
  expect(commands.indexOf('task_secret')).toBeGreaterThanOrEqual(0)
  expect(commands.indexOf('task_secret')).toBeLessThan(commands.indexOf('task_add_provider'))
  expect(api.invoke).toHaveBeenCalledWith('task_secret', { name: 'anthropic', value: 'sk-ant-test' })
  const [, args] = api.invoke.mock.calls.find(([command]) => command === 'task_add_provider')!
  expect((args as { setup: { model: string } }).setup.model).toBe('claude-sonnet-5')
  wrapper.unmount()
})

it('lists only apps this machine could run', async () => {
  const wrapper = mount(ProvidersPanel)
  await flushPromises()
  const names = wrapper.findAll('.provider-item h3').map(h => h.text())
  expect(names).toContain('Apple Intelligence')
  expect(names).not.toContain('Claude Code')
  expect(row(wrapper, 'ChatGPT · Codex').get('.provider-pill').text()).toBe('Not found')
  wrapper.unmount()
})

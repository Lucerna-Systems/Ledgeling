export type ProviderKind = 'none' | 'ollama' | 'openai' | 'anthropic' | 'compatible' | 'codex' | 'claude' | 'apple'
export interface Connection { kind: ProviderKind; model: string; endpoint: string; executable: string }
/** Acceptance of Ledgeling's terms for the on-device model. The backend clamps the
 *  version and stamps the time, so these are a request, not a claim. */
export interface AppleTerms { acceptedVersion: number; acceptedAt: number }
export interface Connections { active: Connection; searchEnabled: boolean; perPet: Record<number, Connection>; appleTerms: AppleTerms }
/** What the machine actually has: installed harnesses and saved API keys. */
export interface ProviderDetection { codexInstalled: boolean; claudeCode: boolean; ollamaApp: boolean; ollamaReachable: boolean; openaiKey: boolean; anthropicKey: boolean; compatibleKey: boolean; braveKey: boolean; appleSupported: boolean; appleReady: boolean; appleDetail: string }
export interface TaskRequest { prompt: string; petId: number | null; roots: string[]; attachments: string[]; preset: string; mode: string; parentId: number | null; allowCloud: boolean; allowWeb: boolean; context?: string }
export interface Operation { kind: string; source: string | null; destination: string; content: string | null; state: string }
/** The backend's plain-words rendering of an approval (`tasks::summary`).
 *  Approval surfaces render this; `details` is the requesting runtime's own
 *  payload and is never shown. */
export interface ApprovalSummary { headline: string; items: string[]; more: number; note: string; accept: string; monospace: boolean }
export interface Approval { id: number; title: string; details: { operations?: Operation[]; [key: string]: unknown }; summary: ApprovalSummary }
/** The pet a task was for, stamped by the backend when it started. */
export interface TaskPet { packId: string; name: string }
export interface Task { id: number; request: TaskRequest; connection: Connection; status: string; progress: string; result: string; error: string | null; approval: Approval | null; operations: Operation[]; sources: { title: string; url: string; excerpt: string }[]; createdAt: number; pet: TaskPet | null; title: string }
export interface Capabilities { models: string[]; structured: boolean; advanced: boolean; detail: string }
export const terminal = (status: string) => ['completed', 'failed', 'cancelled', 'interrupted', 'undone'].includes(status)
export const providerLabels: Record<ProviderKind, string> = {
  none: 'No AI', ollama: 'Ollama', openai: 'OpenAI', anthropic: 'Anthropic', compatible: 'Custom endpoint',
  codex: 'Codex', claude: 'Claude Code', apple: 'Apple Intelligence',
}
/** Built-in actions run locally without AI; `hint` says what the request text means for each. */
export const presets = [
  { value: 'sort_type', label: 'Sort by file type', hint: 'Organise my folder' },
  { value: 'sort_date', label: 'Sort by month modified', hint: 'Organise my folder' },
  { value: 'rename', label: 'Rename in bulk', hint: 'Type the new name prefix' },
  { value: 'find', label: 'Find files', hint: 'What should I look for?' },
  { value: 'duplicates', label: 'Find duplicates', hint: 'Find exact duplicates — nothing is deleted' },
] as const

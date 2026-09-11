export type ProviderKind = 'none' | 'ollama' | 'openai' | 'anthropic' | 'compatible' | 'codex' | 'claude'
export interface Connection { kind: ProviderKind; model: string; endpoint: string; executable: string }
export interface Connections { active: Connection; searchEnabled: boolean; perPet: Record<number, Connection> }
/** What the machine actually has: installed harnesses and saved API keys. */
export interface ProviderDetection { codexInstalled: boolean; claudeBridge: boolean; ollamaApp: boolean; ollamaReachable: boolean; openaiKey: boolean; anthropicKey: boolean; compatibleKey: boolean; braveKey: boolean }
export interface TaskRequest { prompt: string; petId: number | null; roots: string[]; attachments: string[]; preset: string; mode: string; parentId: number | null; allowCloud: boolean; allowWeb: boolean }
export interface Operation { kind: string; source: string | null; destination: string; content: string | null; state: string }
/** The backend's plain-words rendering of an approval (`tasks::summary`).
 *  Approval surfaces render this; `details` is the requesting runtime's own
 *  payload and is never shown. */
export interface ApprovalSummary { headline: string; items: string[]; more: number; note: string; accept: string; monospace: boolean }
export interface Approval { id: number; title: string; details: { operations?: Operation[]; [key: string]: unknown }; summary: ApprovalSummary }
export interface Task { id: number; request: TaskRequest; connection: Connection; status: string; progress: string; result: string; error: string | null; approval: Approval | null; operations: Operation[]; sources: { title: string; url: string; excerpt: string }[]; createdAt: number }
export interface Capabilities { models: string[]; structured: boolean; advanced: boolean; detail: string }
export const terminal = (status: string) => ['completed', 'failed', 'cancelled', 'interrupted', 'undone'].includes(status)

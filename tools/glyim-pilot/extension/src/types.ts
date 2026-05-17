export const PROTOCOL_VERSION = 1;

export interface SessionReady { type: 'session.ready'; sessionId: string; providerId: string; tabId: number; traceId?: string; v: number; }
export interface OpsReady { type: 'ops.ready'; sessionId: string; content: string; turn: number; traceId?: string; v: number; }
export interface StreamComplete { type: 'stream.complete'; sessionId: string; turn: number; fullResponse: string; traceId?: string; v: number; }
export interface ErrorDetected { type: 'error.detected'; sessionId: string; errorType: string; errorMessage: string; recoverable: boolean; traceId?: string; v: number; }
export interface Pong { type: 'pong'; timestamp: number; v: number; }
export type ExtensionMessage = SessionReady | OpsReady | StreamComplete | ErrorDetected | Pong;

export interface SessionStart { type: 'session.start'; sessionId: string; providerId: string; prompt: string; systemPrompt: string; traceId?: string; v: number; }
export interface FeedbackSend { type: 'feedback.send'; sessionId: string; message: string; turn: number; traceId?: string; v: number; }
export interface FeedbackContinue { type: 'feedback.continue'; sessionId: string; traceId?: string; v: number; }
export interface RetryPrompt { type: 'retry.prompt'; sessionId: string; message: string; delay: number; traceId?: string; v: number; }
export interface SessionPause { type: 'session.pause'; sessionId: string; traceId?: string; v: number; }
export interface SessionAbort { type: 'session.abort'; sessionId: string; traceId?: string; v: number; }
export interface Ping { type: 'ping'; timestamp: number; v: number; }
export type CliMessage = SessionStart | FeedbackSend | FeedbackContinue | RetryPrompt | SessionPause | SessionAbort | Ping;

export interface TabSession { tabId: number; sessionId: string; streamId: string; providerId: string; status: 'active' | 'paused' | 'error'; turn: number; }

export const DANGEROUS_PATTERNS: readonly string[] = ['rm -rf', 'git push', 'git reset --hard', 'cargo publish', 'sudo', 'chmod 777', 'mkfs', 'dd if='];

export function containsDangerousPattern(content: string): string | null {
  const lower = content.toLowerCase();
  for (const pattern of DANGEROUS_PATTERNS) { if (lower.includes(pattern.toLowerCase())) return pattern; }
  return null;
}

export function normalizeLineEndings(text: string): string { return text.replace(/\r/g, ''); }

export function validateMessageVersion(v: number | undefined): string | null {
  if (v === undefined || v === 0) return `message with v=${v ?? 'undefined'} rejected — protocol version required (current: ${PROTOCOL_VERSION})`;
  if (v > PROTOCOL_VERSION) return `message version ${v} > server version ${PROTOCOL_VERSION} — may not work`;
  return null;
}

export function serializeTabSessions(sessions: Map<number, TabSession>): string {
  const obj: Record<string, TabSession> = {};
  for (const [tabId, session] of sessions.entries()) { obj[String(tabId)] = session; }
  return JSON.stringify(obj);
}

export function deserializeTabSessions(raw: unknown): Map<number, TabSession> {
  const result = new Map<number, TabSession>();
  if (typeof raw !== 'object' || raw === null) return result;
  const obj = raw as Record<string, unknown>;
  for (const [key, value] of Object.entries(obj)) {
    const tabId = Number(key);
    if (!Number.isFinite(tabId)) continue;
    if (typeof value === 'object' && value !== null) result.set(tabId, value as TabSession);
  }
  return result;
}

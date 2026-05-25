import './providers/index';
import { WsClient } from './ws_client';
import { getAllAdapters } from './providers/adapter';
import type { CliMessage, TabSession } from './types';
import { PROTOCOL_VERSION, validateMessageVersion, serializeTabSessions, deserializeTabSessions } from './types';

const ws = new WsClient();
const tabSessions = new Map<number, TabSession>();

ws.onMessage(async (msg: CliMessage) => {
  const versionError = validateMessageVersion((msg as any).v as number | undefined);
  if (versionError) console.warn(`glyim-pilot: ${versionError}`);
  try {
    switch (msg.type) {
      case 'session.start': await handleSessionStart(msg); break;
      case 'feedback.send': await handleFeedbackSend(msg); break;
      case 'feedback.continue': await handleFeedbackContinue(msg); break;
      case 'retry.prompt': await handleRetryPrompt(msg); break;
      case 'session.pause': await handleSessionPause(msg); break;
      case 'session.abort': await handleSessionAbort(msg); break;
      case 'ping': ws.send({ type: 'pong', timestamp: Date.now(), v: PROTOCOL_VERSION }); break;
    }
  } catch (e) { console.warn(`glyim-pilot: error handling ${msg.type}:`, e); }
});

ws.onStatusChange(async (connected) => { if (connected) await restoreSessions(); });
ws.connect();

async function waitForInputElement(tabId: number, maxWaitMs = 10000): Promise<boolean> {
  for (let i = 0; i < maxWaitMs / 200; i++) {
    try {
      const results = await chrome.scripting.executeScript({ target: { tabId }, func: () => !!document.querySelector('textarea, [contenteditable="true"]') });
      if (results[0]?.result) return true;
    } catch { /* tab not ready */ }
    await new Promise(r => setTimeout(r, 200));
  }
  return false;
}

async function injectPrompt(tabId: number, prompt: string): Promise<{ success: boolean; error?: string }> {
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      func: (text: string) => {
        console.log("[glyim-pilot] injection started");
        const input = document.querySelector<HTMLElement>('textarea, [contenteditable="true"]');
        if (!input) return { success: false, error: 'input element not found' };
        input.focus();
        if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
          const start = input.selectionStart ?? 0;
          const end = input.selectionEnd ?? 0;
          input.setRangeText(text, start, end, 'end');
          input.dispatchEvent(new Event('input', { bubbles: true }));
        } else if (input.isContentEditable) {
          document.execCommand('insertText', false, text);
        }
        console.log("[glyim-pilot] text inserted");

        let attempts = 0;
        const maxAttempts = 50;
        const tryClick = () => {
          const btn = document.querySelector<HTMLElement>("button.send-button");
          console.log(`[glyim-pilot] attempt ${attempts + 1} - button:`, btn);
          if (btn) {
            console.log(`  disabled: ${btn.disabled}`);
            console.log(`  aria-disabled: ${btn.getAttribute('aria-disabled')}`);
            console.log(`  visible: ${btn.offsetParent !== null}`);
            console.log(`  rect: ${btn.getBoundingClientRect().width}x${btn.getBoundingClientRect().height}`);
          }
          if (btn && !btn.disabled && btn.getAttribute('aria-disabled') !== 'true') {
            // Try multiple click methods
            console.log("[glyim-pilot] trying .click()");
            btn.click();
            // Also try event dispatch
            const clickEv = new MouseEvent('click', { bubbles: true, cancelable: true, view: window });
            btn.dispatchEvent(clickEv);
            console.log("[glyim-pilot] dispatched MouseEvent");
            // Try focusing and triggering a mousedown+mouseup
            btn.focus();
            btn.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
            btn.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
            console.log("[glyim-pilot] mousedown/mouseup dispatched");
            return;
          }
          if (attempts++ < maxAttempts) {
            setTimeout(tryClick, 200);
          } else {
            console.error("[glyim-pilot] Failed to click after 10 seconds");
          }
        };
        // Start checking after 1 second
        setTimeout(tryClick, 1000);
        return { success: true };
      },
      args: [prompt],
    });
    return results[0]?.result as { success: boolean; error?: string } ?? { success: false, error: 'no result' };
  } catch (e) {
    return { success: false, error: String(e) };
  }
}
async function handleSessionStart(msg: Extract<CliMessage, { type: 'session.start' }>) {
  const { sessionId, providerId, prompt, systemPrompt, traceId } = msg;
  const adapter = getAllAdapters().find(a => a.id === providerId);
  if (!adapter) { console.warn(`glyim-pilot: no adapter for ${providerId}`); return; }
  const tab = await chrome.tabs.create({ url: adapter.homepageUrl, active: true });
  if (!tab.id) return;
  const ready = await waitForInputElement(tab.id);
  if (!ready) { ws.send({ type: 'error.detected', sessionId, errorType: 'input_not_found', errorMessage: 'Input element not found', recoverable: false, v: PROTOCOL_VERSION }); return; }
  const fullPrompt = systemPrompt ? `${systemPrompt}\n\n${prompt}` : prompt;
  const result = await injectPrompt(tab.id, fullPrompt);
  if (!result.success) { ws.send({ type: 'error.detected', sessionId, errorType: 'injection_failed', errorMessage: result.error ?? 'unknown', recoverable: true, v: PROTOCOL_VERSION }); return; }
  tabSessions.set(tab.id, { tabId: tab.id, sessionId, streamId: sessionId, providerId, status: 'active', turn: 0 });
  await persistSessions();
  ws.send({ type: 'session.ready', sessionId, providerId, tabId: tab.id, traceId, v: PROTOCOL_VERSION });
  // Tell content script to start watching for responses (no DOM in background!)
  chrome.tabs.sendMessage(tab.id, { type: 'startWatcher', data: { sessionId, providerId, turn: 0 } });
}

async function handleFeedbackSend(msg: Extract<CliMessage, { type: 'feedback.send' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], msg.message);
  chrome.tabs.sendMessage(entry[0], { type: 'resetWatcherTurn' });
}

async function handleFeedbackContinue(msg: Extract<CliMessage, { type: 'feedback.continue' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], 'Please continue.');
  chrome.tabs.sendMessage(entry[0], { type: 'resetWatcherTurn' });
}

async function handleRetryPrompt(msg: Extract<CliMessage, { type: 'retry.prompt' }>) {
  await new Promise(r => setTimeout(r, msg.delay));
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], msg.message);
}

async function handleSessionPause(msg: Extract<CliMessage, { type: 'session.pause' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  entry[1].status = 'paused';
  chrome.tabs.sendMessage(entry[0], { type: 'stopWatcher' });
  await persistSessions();
}

async function handleSessionAbort(msg: Extract<CliMessage, { type: 'session.abort' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  chrome.tabs.sendMessage(entry[0], { type: 'stopWatcher' });
  tabSessions.delete(entry[0]);
  await persistSessions();
}

function findSession(sessionId: string): [number, TabSession] | null {
  for (const [tabId, sess] of tabSessions.entries()) { if (sess.sessionId === sessionId) return [tabId, sess]; }
  return null;
}

async function persistSessions() { await chrome.storage.local.set({ tabSessions: serializeTabSessions(tabSessions) }); }

async function restoreSessions() {
  const stored = await chrome.storage.local.get('tabSessions');
  if (!stored.tabSessions) return;
  try {
    const sessions = deserializeTabSessions(JSON.parse(stored.tabSessions as string));
    for (const [tabId, sess] of sessions.entries()) {
      try { await chrome.tabs.get(tabId); tabSessions.set(tabId, sess); }
      catch { /* tab gone */ }
    }
  } catch (e) { console.warn('glyim-pilot: failed to restore sessions:', e); }
}

chrome.runtime.onStartup.addListener(restoreSessions);

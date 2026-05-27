import './providers/index';
import { WsClient } from './ws_client';
import { getAllAdapters } from './providers/adapter';
import type { CliMessage, TabSession } from './types';
import { PROTOCOL_VERSION, validateMessageVersion, serializeTabSessions, deserializeTabSessions } from './types';
import { extractGlyimOpsBlocks } from './code_extractor';

const ws = new WsClient();
const tabSessions = new Map<number, TabSession>();

ws.onMessage(async (msg: CliMessage) => {
  console.log('[bg] Received message from server:', msg);
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
        const input = document.querySelector<HTMLElement>('textarea, [contenteditable="true"]');
        if (!input) return { success: false, error: 'input element not found' };
        input.focus();
        if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
          const start = input.selectionStart ?? 0;
          const end = input.selectionEnd ?? 0;
          input.setRangeText(text, start, end, 'end');
          input.dispatchEvent(new Event('input', { bubbles: true }));
          input.dispatchEvent(new Event('change', { bubbles: true }));
        } else if (input.isContentEditable) {
          document.execCommand('insertText', false, text);
        }
        return { success: true };
      },
      args: [prompt],
    });
    return results[0]?.result as { success: boolean; error?: string } ?? { success: false, error: 'no result' };
  } catch (e) { return { success: false, error: String(e) }; }
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

  // Use adapter's selectors (no provider‑specific branches)
  const sendSelector = (adapter as any).getSendSelector ? (adapter as any).getSendSelector() : "button[type='submit']";
  const completionSelector = (adapter as any).getCompletionSelector ? (adapter as any).getCompletionSelector() : "button[aria-label*='Copy'], button[aria-label*='copy']";
  const assistantSelector = adapter.assistantSelector;

  await chrome.scripting.executeScript({
    target: { tabId: tab.id },
    func: (sendSel: string, completionSel: string, asstSel: string, sid: string, turnNum: number) => {
      console.log('[injected] Script started');

      let clickAttempts = 0;
      const maxAttempts = 50;
      const tryClick = () => {
        const btn = document.querySelector<HTMLElement>(sendSel);
        if (btn && !btn.disabled && btn.getAttribute('aria-disabled') !== 'true' && btn.offsetParent !== null) {
          btn.scrollIntoView({ block: 'center' });
          btn.focus();
          const rect = btn.getBoundingClientRect();
          const centerX = rect.left + rect.width / 2;
          const centerY = rect.top + rect.height / 2;
          const mousedown = new MouseEvent('mousedown', { bubbles: true, cancelable: true, view: window, clientX: centerX, clientY: centerY });
          const mouseup = new MouseEvent('mouseup', { bubbles: true, cancelable: true, view: window, clientX: centerX, clientY: centerY });
          const click = new MouseEvent('click', { bubbles: true, cancelable: true, view: window, clientX: centerX, clientY: centerY });
          btn.dispatchEvent(mousedown);
          btn.dispatchEvent(mouseup);
          btn.dispatchEvent(click);
          btn.click();
          console.log('[injected] Send button clicked');

          const observer = new MutationObserver((mutations) => {
            for (const mutation of mutations) {
              if (mutation.type === 'childList') {
                for (const node of mutation.addedNodes) {
                  if (node.nodeType === Node.ELEMENT_NODE) {
                    const element = node as Element;
                    let completionEl: Element | null = null;
                    if (element.matches?.(completionSel)) completionEl = element;
                    else completionEl = element.querySelector?.(completionSel);
                    if (completionEl) {
                      console.log('[injected] Completion detected');
                      // Extract response using assistantSelector (no provider logic)
                      const responseElement = document.querySelector(asstSel);
                      let fullResponse = responseElement ? responseElement.textContent || '' : '';
                      // If empty, retry on next microtask (allows Monaco editor to populate)
                      if (!fullResponse) {
                        Promise.resolve().then(() => {
                          const retryElement = document.querySelector(asstSel);
                          const retryResponse = retryElement ? retryElement.textContent || '' : '';
                          window.postMessage({
                            type: 'stream_complete',
                            sessionId: sid,
                            turn: turnNum,
                            fullResponse: retryResponse
                          }, '*');
                          observer.disconnect();
                        });
                      } else {
                        window.postMessage({
                          type: 'stream_complete',
                          sessionId: sid,
                          turn: turnNum,
                          fullResponse: fullResponse
                        }, '*');
                        observer.disconnect();
                      }
                      return;
                    }
                  }
                }
              }
            }
          });
          observer.observe(document.body, { childList: true, subtree: true });
          console.log('[injected] Observer started');
          return;
        }
        if (clickAttempts++ < maxAttempts) setTimeout(tryClick, 200);
        else console.error('[injected] Failed to click send button');
      };
      setTimeout(tryClick, 1500);
    },
    args: [sendSelector, completionSelector, assistantSelector, sessionId, 0],
  });
}

// --- Generic helper for feedback/continue/retry (no provider‑specific code) ---
async function reinjectWatcher(tabId: number, adapter: any, sessionId: string, turn: number) {
  const sendSelector = (adapter as any).getSendSelector ? (adapter as any).getSendSelector() : "button[type='submit']";
  const assistantSelector = adapter.assistantSelector;
  await chrome.scripting.executeScript({
    target: { tabId },
    func: (sendSel: string, asstSel: string, sid: string, turnNum: number) => {
      let attempts = 0;
      const tryClick = () => {
        const btn = document.querySelector<HTMLElement>(sendSel);
        if (btn && !btn.hasAttribute('disabled') && btn.getAttribute('aria-disabled') !== 'true') {
          btn.click();
          return;
        }
        if (attempts++ < 50) setTimeout(tryClick, 200);
      };
      setTimeout(tryClick, 500);
      const obs = new MutationObserver(() => {
        const responseElement = document.querySelector(asstSel);
        if (responseElement) {
          const full = responseElement.textContent || '';
          chrome.runtime.sendMessage({ type: 'stream.complete', sessionId: sid, turn: turnNum, fullResponse: full });
          obs.disconnect();
        }
      });
      obs.observe(document.body, { childList: true, subtree: true });
    },
    args: [sendSelector, assistantSelector, sessionId, turn],
  });
}

async function handleFeedbackSend(msg: Extract<CliMessage, { type: 'feedback.send' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], msg.message);
  const adapter = getAllAdapters().find(a => a.id === entry[1].providerId);
  if (adapter) {
    await reinjectWatcher(entry[0], adapter, entry[1].sessionId, entry[1].turn + 1);
  }
  const sess = tabSessions.get(entry[0]);
  if (sess) sess.turn++;
  await persistSessions();
}

async function handleFeedbackContinue(msg: Extract<CliMessage, { type: 'feedback.continue' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], 'Please continue.');
  const adapter = getAllAdapters().find(a => a.id === entry[1].providerId);
  if (adapter) {
    await reinjectWatcher(entry[0], adapter, entry[1].sessionId, entry[1].turn + 1);
  }
  const sess = tabSessions.get(entry[0]);
  if (sess) sess.turn++;
  await persistSessions();
}

async function handleRetryPrompt(msg: Extract<CliMessage, { type: 'retry.prompt' }>) {
  await new Promise(r => setTimeout(r, msg.delay));
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], msg.message);
  const adapter = getAllAdapters().find(a => a.id === entry[1].providerId);
  if (adapter) {
    await reinjectWatcher(entry[0], adapter, entry[1].sessionId, entry[1].turn + 1);
  }
  const sess = tabSessions.get(entry[0]);
  if (sess) sess.turn++;
  await persistSessions();
}

async function handleSessionPause(msg: Extract<CliMessage, { type: 'session.pause' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  entry[1].status = 'paused';
  await persistSessions();
}

async function handleSessionAbort(msg: Extract<CliMessage, { type: 'session.abort' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
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
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  console.log('[bg] Received runtime message:', message);
  if (message.type === 'stream.complete') {
    console.log('[bg] Processing stream.complete for', message.sessionId);
    const blocks = extractGlyimOpsBlocks(message.fullResponse);
    console.log(`[bg] Extracted ${blocks.length} ops blocks`);
    for (const block of blocks) {
      const success = ws.send({
        type: 'ops.ready',
        sessionId: message.sessionId,
        content: block,
        turn: message.turn,
        v: PROTOCOL_VERSION
      });
      console.log(`[bg] Sent ops.ready, success=${success}`);
    }
    sendResponse({ received: true });
    return true;
  }
  return false;
});

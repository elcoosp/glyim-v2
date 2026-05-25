import './providers/index';
import { WsClient } from './ws_client';
import { getAllAdapters } from './providers/adapter';
import { StreamWatcher } from './stream_watcher';
import type { CliMessage, TabSession } from './types';
import { PROTOCOL_VERSION, validateMessageVersion, serializeTabSessions, deserializeTabSessions } from './types';

const ws = new WsClient();
const tabSessions = new Map<number, TabSession>();
const watchers = new Map<number, StreamWatcher>();

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

async function waitForPageLoad(tabId: number, timeoutMs = 10000): Promise<boolean> {
  return new Promise((resolve) => {
    const listener = (updatedTabId: number, changeInfo: chrome.tabs.TabChangeInfo) => {
      if (updatedTabId === tabId && changeInfo.status === 'complete') {
        chrome.tabs.onUpdated.removeListener(listener);
        resolve(true);
      }
    };
    chrome.tabs.onUpdated.addListener(listener);
    setTimeout(() => {
      chrome.tabs.onUpdated.removeListener(listener);
      resolve(false);
    }, timeoutMs);
  });
}

async function waitForInputElement(tabId: number, selector: string, maxWaitMs = 10000): Promise<boolean> {
  console.log(`Waiting for input selector: ${selector}`);
  const start = Date.now();
  while (Date.now() - start < maxWaitMs) {
    try {
      const results = await chrome.scripting.executeScript({
        target: { tabId },
        func: (sel) => !!document.querySelector(sel),
        args: [selector]
      });
      if (results[0]?.result) return true;
    } catch (e) { /* not ready */ }
    await new Promise(r => setTimeout(r, 200));
  }
  return false;
}

async function injectPrompt(tabId: number, selector: string, prompt: string, sendSelector: string): Promise<{ success: boolean; error?: string }> {
  console.log(`Injecting prompt into ${selector}`);
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      func: (inputSel, text, sendSel) => {
        const input = document.querySelector<HTMLElement>(inputSel);
        if (!input) {
          console.error(`Input not found: ${inputSel}`);
          return { success: false, error: 'input element not found' };
        }
        input.focus();
        if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
          const start = input.selectionStart ?? 0;
          const end = input.selectionEnd ?? 0;
          input.setRangeText(text, start, end, 'end');
          input.dispatchEvent(new Event('input', { bubbles: true }));
        } else if (input.isContentEditable) {
          document.execCommand('insertText', false, text);
        }
        const pollForSend = (): void => {
          const btn = document.querySelector<HTMLElement>(sendSel);
          if (btn && !btn.hasAttribute('disabled') && btn.getAttribute('aria-disabled') !== 'true') {
            btn.click();
            return;
          }
          setTimeout(pollForSend, 100);
        };
        setTimeout(pollForSend, 50);
        return { success: true };
      },
      args: [selector, prompt, sendSelector],
    });
    return results[0]?.result as { success: boolean; error?: string } ?? { success: false, error: 'no result' };
  } catch (e) { return { success: false, error: String(e) }; }
}

async function handleSessionStart(msg: Extract<CliMessage, { type: 'session.start' }>) {
  console.log("handleSessionStart", msg);
  const { sessionId, providerId, prompt, systemPrompt, traceId } = msg;
  const adapter = getAllAdapters().find(a => a.id === providerId);
  if (!adapter) { console.warn(`glyim-pilot: no adapter for ${providerId}`); return; }
  const tab = await chrome.tabs.create({ url: adapter.homepageUrl, active: true });
  if (!tab.id) return;

  // Wait for page to load completely
  const pageLoaded = await waitForPageLoad(tab.id, 15000);
  if (!pageLoaded) {
    ws.send({ type: 'error.detected', sessionId, errorType: 'page_load_timeout', errorMessage: 'Page did not load within 15s', recoverable: false, v: PROTOCOL_VERSION });
    return;
  }

  // Determine selectors
  let inputSelector: string;
  let sendSelector: string;
  if (providerId === 'zai') {
    inputSelector = '#chat-input';
    sendSelector = "button[type='submit']";
  } else if (providerId === 'deepseek') {
    inputSelector = '#root > div > div.cb86951c > div.c3ecdb44 > div._7780f2e > div > div > div._9a2f8e4 > div.aaff8b8f > div > div > div._24fad49 > textarea';
    sendSelector = "#root > div > div.cb86951c > div.c3ecdb44 > div._7780f2e > div > div > div._9a2f8e4 > div.aaff8b8f > div > div > div.ec4f5d61 > div.bf38813a > div:nth-child(3) > div";
  } else {
    inputSelector = adapter.config?.inputSelector || 'textarea';
    sendSelector = "button[type='submit']";
  }

  const inputReady = await waitForInputElement(tab.id, inputSelector, 10000);
  if (!inputReady) {
    ws.send({ type: 'error.detected', sessionId, errorType: 'input_not_found', errorMessage: `Input '${inputSelector}' not found`, recoverable: false, v: PROTOCOL_VERSION });
    return;
  }

  const fullPrompt = systemPrompt ? `${systemPrompt}\n\n${prompt}` : prompt;
  const result = await injectPrompt(tab.id, inputSelector, fullPrompt, sendSelector);
  if (!result.success) {
    ws.send({ type: 'error.detected', sessionId, errorType: 'injection_failed', errorMessage: result.error ?? 'unknown', recoverable: true, v: PROTOCOL_VERSION });
    return;
  }

  tabSessions.set(tab.id, { tabId: tab.id, sessionId, streamId: sessionId, providerId, status: 'active', turn: 0 });
  await persistSessions();
  ws.send({ type: 'session.ready', sessionId, providerId, tabId: tab.id, traceId, v: PROTOCOL_VERSION });
  startWatcher(tab.id, sessionId, adapter);
}

// The rest of the functions (unchanged from original, but we must include them)
async function handleFeedbackSend(msg: Extract<CliMessage, { type: 'feedback.send' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  const providerId = entry[1].providerId;
  let inputSelector: string, sendSelector: string;
  if (providerId === 'zai') {
    inputSelector = '#chat-input';
    sendSelector = "button[type='submit']";
  } else {
    inputSelector = 'textarea';
    sendSelector = "button[type='submit']";
  }
  await injectPrompt(entry[0], inputSelector, msg.message, sendSelector);
  watchers.get(entry[0])?.resetForNewTurn();
}

async function handleFeedbackContinue(msg: Extract<CliMessage, { type: 'feedback.continue' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  const providerId = entry[1].providerId;
  let inputSelector: string, sendSelector: string;
  if (providerId === 'zai') {
    inputSelector = '#chat-input';
    sendSelector = "button[type='submit']";
  } else {
    inputSelector = 'textarea';
    sendSelector = "button[type='submit']";
  }
  await injectPrompt(entry[0], inputSelector, 'Please continue.', sendSelector);
  watchers.get(entry[0])?.resetForNewTurn();
}

async function handleRetryPrompt(msg: Extract<CliMessage, { type: 'retry.prompt' }>) {
  await new Promise(r => setTimeout(r, msg.delay));
  const entry = findSession(msg.sessionId); if (!entry) return;
  const providerId = entry[1].providerId;
  let inputSelector: string, sendSelector: string;
  if (providerId === 'zai') {
    inputSelector = '#chat-input';
    sendSelector = "button[type='submit']";
  } else {
    inputSelector = 'textarea';
    sendSelector = "button[type='submit']";
  }
  await injectPrompt(entry[0], inputSelector, msg.message, sendSelector);
}

async function handleSessionPause(msg: Extract<CliMessage, { type: 'session.pause' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  entry[1].status = 'paused'; watchers.get(entry[0])?.stop(); await persistSessions();
}

async function handleSessionAbort(msg: Extract<CliMessage, { type: 'session.abort' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  watchers.get(entry[0])?.stop(); watchers.delete(entry[0]); tabSessions.delete(entry[0]); await persistSessions();
}

function startWatcher(tabId: number, sessionId: string, adapter: ReturnType<typeof getAllAdapters>[0]) {
  watchers.get(tabId)?.stop();
  const watcher = new StreamWatcher(adapter, sessionId,
    (content, turn) => ws.send({ type: 'ops.ready', sessionId, content, turn, v: PROTOCOL_VERSION }),
    (full, turn) => ws.send({ type: 'stream.complete', sessionId, turn, fullResponse: full, v: PROTOCOL_VERSION }),
    (content, pattern) => ws.send({ type: 'error.detected', sessionId, errorType: 'dangerous_pattern', errorMessage: `Dangerous: "${pattern}"`, recoverable: true, v: PROTOCOL_VERSION }),
  );
  watcher.start(); watchers.set(tabId, watcher);
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
      try { await chrome.tabs.get(tabId); tabSessions.set(tabId, sess); const adapter = getAllAdapters().find(a => a.id === sess.providerId); if (adapter) startWatcher(tabId, sess.sessionId, adapter); }
      catch { /* tab gone */ }
    }
  } catch (e) { console.warn('glyim-pilot: failed to restore sessions:', e); }
}

chrome.runtime.onStartup.addListener(restoreSessions);

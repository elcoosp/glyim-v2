import { getAdapterForUrl } from './providers/adapter';
import { StreamWatcher } from './stream_watcher';

let activeWatcher: StreamWatcher | null = null;

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg.type === 'startWatcher') {
    const adapter = getAdapterForUrl(window.location.href);
    if (!adapter) {
      console.error('glyim-pilot: no adapter for', window.location.href);
      sendResponse({ error: 'no adapter' });
      return true;
    }
    if (activeWatcher) activeWatcher.stop();
    const { sessionId, turn } = msg.data;
    activeWatcher = new StreamWatcher(
      adapter,
      sessionId,
      (content, turnNum) => chrome.runtime.sendMessage({ type: 'ops.ready', sessionId, content, turn: turnNum }),
      (full, turnNum) => chrome.runtime.sendMessage({ type: 'stream.complete', sessionId, turn: turnNum, fullResponse: full }),
      (content, pattern) => chrome.runtime.sendMessage({ type: 'error.detected', sessionId, errorType: 'dangerous_pattern', errorMessage: pattern, recoverable: true })
    );
    activeWatcher.start();
    sendResponse({ success: true });
  } else if (msg.type === 'stopWatcher') {
    if (activeWatcher) activeWatcher.stop();
    activeWatcher = null;
    sendResponse({ success: true });
  } else if (msg.type === 'resetWatcherTurn') {
    if (activeWatcher) activeWatcher.resetForNewTurn();
    sendResponse({ success: true });
  }
  return true;
});

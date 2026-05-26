console.log('[content] Content script loaded for', window.location.href);

window.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'stream_complete') {
    console.log('[content] Forwarding stream_complete to background');
    chrome.runtime.sendMessage({
      type: 'stream.complete',
      sessionId: event.data.sessionId,
      turn: event.data.turn,
      fullResponse: event.data.fullResponse
    }, (response) => {
      if (chrome.runtime.lastError) {
        console.error('[content] sendMessage error:', chrome.runtime.lastError.message);
      } else {
        console.log('[content] sendMessage success, response:', response);
      }
    });
  }
});

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg.type === 'content.checkStatus') {
    sendResponse({ streaming: !!document.querySelector('.typing-indicator, .streaming, .loading, mat-progress-bar'), offline: !navigator.onLine });
  }
  if (msg.type === 'content.injectPrompt') {
    const prompt = msg.prompt as string;
    const input = document.querySelector<HTMLElement>('textarea, [contenteditable="true"]');
    if (!input) { sendResponse({ success: false, error: 'input not found' }); return true; }
    input.focus();
    if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
      const start = input.selectionStart ?? 0;
      const end = input.selectionEnd ?? 0;
      input.setRangeText(prompt, start, end, 'end');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (input.isContentEditable) {
      document.execCommand('insertText', false, prompt);
    }
    sendResponse({ success: true });
  }
  return true;
});

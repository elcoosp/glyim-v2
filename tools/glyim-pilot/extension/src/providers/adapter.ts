export interface ProviderError {
  type: 'rate_limit' | 'server_busy' | 'capacity' | 'server_error' | 'network_error';
  message: string;
  recoverable: boolean;
}

export interface ProviderAdapter {
  readonly id: string;
  readonly urlPattern: RegExp;
  readonly assistantSelector: string;
  readonly homepageUrl: string;
  setInput(text: string): Promise<void>;
  submitMessage(): Promise<void>;
  isStreaming(): boolean;
  getCodeBlocks(): string[];
  detectError(): ProviderError | null;
  getAssistantText(): string;
}

export interface ProviderConfig {
  id: string;
  urlPattern: RegExp;
  homepageUrl: string;
  inputSelector: string;
  assistantSelector: string;
  streamingSelector: string;
  errorSelectors: string[];
  customSetInput?: (text: string) => Promise<void>;
}

const adapterRegistry: ProviderAdapter[] = [];
export function registerAdapter(adapter: ProviderAdapter): void { adapterRegistry.push(adapter); }
export function getAdapterForUrl(url: string): ProviderAdapter | null {
  return adapterRegistry.find(a => a.urlPattern.test(url)) ?? null;
}
export function getAllAdapters(): ProviderAdapter[] { return [...adapterRegistry]; }

export function insertText(element: HTMLTextAreaElement | HTMLInputElement, text: string): void {
  const start = element.selectionStart ?? 0;
  const end = element.selectionEnd ?? 0;
  element.setRangeText(text, start, end, 'end');
  element.dispatchEvent(new Event('input', { bubbles: true }));
}

export async function clickSendWhenEnabled(maxWaitMs = 5000): Promise<void> {
  const pollInterval = 100;
  const maxAttempts = maxWaitMs / pollInterval;
  for (let i = 0; i < maxAttempts; i++) {
    const btn = document.querySelector<HTMLButtonElement>(
      "button[type='submit'], button[aria-label*='send'], div[class*='send-button']"
    );
    if (btn && !btn.disabled && !btn.getAttribute('aria-disabled')) { btn.click(); return; }
    await new Promise(r => setTimeout(r, pollInterval));
  }
  throw new Error('send button not found or not enabled within timeout');
}

export async function setInputText(selector: string, text: string): Promise<void> {
  const element = document.querySelector<HTMLElement>(selector);
  if (!element) throw new Error(`input not found by selector: ${selector}`);
  element.focus();

  if (element instanceof HTMLTextAreaElement || element instanceof HTMLInputElement) {
    insertText(element, text);
  } else if (element.isContentEditable) {
    document.execCommand('insertText', false, text);
  } else {
    document.execCommand('insertText', false, text);
  }
}

export class ConfigurableAdapter implements ProviderAdapter {
  readonly id: string;
  readonly urlPattern: RegExp;
  readonly assistantSelector: string;
  readonly homepageUrl: string;
  private readonly config: ProviderConfig;

  constructor(config: ProviderConfig) {
    this.config = config;
    this.id = config.id;
    this.urlPattern = config.urlPattern;
    this.assistantSelector = config.assistantSelector;
    this.homepageUrl = config.homepageUrl;
  }

  async setInput(text: string): Promise<void> {
    if (this.config.customSetInput) {
      await this.config.customSetInput(text);
      return;
    }
    await setInputText(this.config.inputSelector, text);
  }

  async submitMessage(): Promise<void> { await clickSendWhenEnabled(); }
  isStreaming(): boolean {
    // If explicit streaming selector present, assume streaming
    if (this.config.streamingSelector && document.querySelector(this.config.streamingSelector)) {
        return true;
    }
    // Get the latest assistant message container
    const lastMsg = document.querySelector(`${this.assistantSelector}:last-of-type`);
    if (!lastMsg) return true; // no message yet
    // Look for a stop button (still generating)
    const stopBtn = document.querySelector('button[aria-label="Stop"], button[aria-label*="stop"], [role="button"][aria-label*="stop"]');
    if (stopBtn && !stopBtn.hasAttribute('disabled') && stopBtn.getAttribute('aria-disabled') !== 'true') {
        return true;
    }
    // Look for copy button inside the last message – indicates completion
    const copyBtn = lastMsg.querySelector('button[aria-label*="Copy"], button[aria-label*="copy"], [class*="copy"]');
    if (copyBtn) {
        return false;
    }
    // Look for edit/regenerate buttons
    const editBtn = lastMsg.querySelector('button[aria-label*="Edit"], button[aria-label*="edit"]');
    if (editBtn) return false;
    const regenBtn = document.querySelector('button[aria-label*="Regenerate"], button[aria-label*="regen"]');
    if (regenBtn && lastMsg.textContent?.trim()) return false;
    // If input became enabled and there is text, assume done
    const input = document.querySelector(this.config.inputSelector);
    if (input && (input as HTMLInputElement).disabled === false && lastMsg.textContent?.trim()) {
        return false;
    }
    // Fallback to original streaming indicator
    return document.querySelector(this.config.streamingSelector) !== null;
}
  getCodeBlocks(): string[] { return Array.from(document.querySelectorAll('pre code')).map(b => b.textContent ?? ''); }

  detectError(): ProviderError | null {
    for (const selector of this.config.errorSelectors) {
      // Convert NodeList to array to satisfy TypeScript iterator constraint
      const elements = Array.from(document.querySelectorAll(selector));
      for (const el of elements) {
        if (el.closest(this.assistantSelector)) continue;
        const text = el.textContent?.toLowerCase() ?? '';
        if (text.includes('rate limit') || text.includes('too frequent'))
          return { type: 'rate_limit', message: el.textContent?.trim() ?? '', recoverable: true };
        if (text.includes('capacity'))
          return { type: 'capacity', message: el.textContent?.trim() ?? '', recoverable: true };
        if (text.includes('server error'))
          return { type: 'server_error', message: el.textContent?.trim() ?? '', recoverable: true };
        if (text.includes('rate') || text.includes('limit'))
          return { type: 'rate_limit', message: el.textContent?.trim() ?? '', recoverable: true };
      }
    }
    return null;
  }

  getAssistantText(): string {
    const sel = this.config.assistantSelector;
    const lastEl = document.querySelector(`${sel}:last-of-type`);
    return lastEl?.textContent ?? '';
  }
}

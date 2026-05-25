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

// DEEPSEEK SPECIFIC SEND BUTTON SELECTOR (your exact div)
const DEEPSEEK_SEND_SELECTOR = "#root > div > div.cb86951c > div.c3ecdb44 > div._7780f2e > div > div > div._9a2f8e4 > div.aaff8b8f > div > div > div.ec4f5d61 > div.bf38813a > div:nth-child(3) > div";

export async function clickSendWhenEnabled(maxWaitMs = 5000): Promise<void> {
  const pollInterval = 100;
  const maxAttempts = maxWaitMs / pollInterval;
  for (let i = 0; i < maxAttempts; i++) {
    // Standard buttons + DeepSeek's stable selector
    const btn = document.querySelector<HTMLElement>(
      "button[type='submit'], button[aria-label*='send'], div[class*='send-button'], div.ds-icon-button[role='button'][aria-disabled='false']"
    );
    if (btn && !btn.hasAttribute('disabled') && btn.getAttribute('aria-disabled') !== 'true') {
      btn.click();
      return;
    }
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

  // ENHANCED isStreaming: detects copy button as completion
  isStreaming(): boolean {
    // If explicit streaming selector present, assume streaming
    if (this.config.streamingSelector && document.querySelector(this.config.streamingSelector)) {
      return true;
    }
    // Get latest assistant message
    const lastMsg = document.querySelector(`${this.assistantSelector}:last-of-type`);
    if (!lastMsg) return true;
    // Look for copy button inside the last message – indicates completion
    const copyBtn = lastMsg.querySelector('button[aria-label*="Copy"], button[aria-label*="copy"], [class*="copy"]');
    if (copyBtn) return false;
    // Look for stop button (still generating)
    const stopBtn = document.querySelector('button[aria-label="Stop"], button[aria-label*="stop"]');
    if (stopBtn && !stopBtn.hasAttribute('disabled')) return true;
    // Fallback to original streaming indicator
    return document.querySelector(this.config.streamingSelector) !== null;
  }

  getCodeBlocks(): string[] { return Array.from(document.querySelectorAll('pre code')).map(b => b.textContent ?? ''); }

  detectError(): ProviderError | null {
    for (const selector of this.config.errorSelectors) {
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
    const lastEl = document.querySelector(`${this.assistantSelector}:last-of-type`);
    return lastEl?.textContent ?? '';
  }
}

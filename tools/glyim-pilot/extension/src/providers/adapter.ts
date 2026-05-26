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
  completionSelector: string;
  sendSelector: string
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

export async function clickSendWhenEnabled(adapter: ConfigurableAdapter, maxWaitMs = 10000): Promise<void> {
  const sendSelector = adapter.getSendSelector();
  console.log(`[adapter] clickSendWhenEnabled: polling for selector "${sendSelector}"`);
  if (!sendSelector) {
    throw new Error('No send selector configured for this provider');
  }
  const pollInterval = 200;
  const maxAttempts = maxWaitMs / pollInterval;
  for (let i = 0; i < maxAttempts; i++) {
    const btn = document.querySelector<HTMLElement>(sendSelector);
    if (btn) {
      btn.click();
      return;
    }
    await new Promise(r => setTimeout(r, pollInterval));
  }
  throw new Error(`Send button not found or not enabled with selector: ${sendSelector}`);
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
  getSendSelector(): string {
    // Return the provider-specific send selector from config, or a fallback
    return this.config.sendSelector || "button[type='submit']";
  }
  getCompletionSelector(): string | undefined {
    return this.config.completionSelector;
  }
  async setInput(text: string): Promise<void> {
    if (this.config.customSetInput) {
      await this.config.customSetInput(text);
      return;
    }
    await setInputText(this.config.inputSelector, text);
  }

  async submitMessage(): Promise<void> {
    console.log(`[adapter] submitMessage called for provider ${this.id}`);
    const sel = this.getSendSelector();
    console.log(`[adapter] sendSelector = "${sel}"`);
    if (!sel) {
      throw new Error(`No send selector for ${this.id}`);
    }
    await clickSendWhenEnabled(this);
  }
  isStreaming(): boolean {
    if (this.config.streamingSelector && document.querySelector(this.config.streamingSelector)) {
      return true;
    }
    const lastMsg = document.querySelector(`${this.assistantSelector}:last-of-type`);
    if (!lastMsg) return true;
    const copyBtn = lastMsg.querySelector('button[aria-label*="Copy"], button[aria-label*="copy"], [class*="copy"]');
    if (copyBtn) return false;
    const stopBtn = document.querySelector('button[aria-label="Stop"], button[aria-label*="stop"]');
    if (stopBtn && !stopBtn.hasAttribute('disabled')) return true;
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

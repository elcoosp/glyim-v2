import { ConfigurableAdapter, registerAdapter, setInputText } from './adapter';

registerAdapter(new ConfigurableAdapter({
  id: 'deepseek',
  urlPattern: /chat\.deepseek\.com/,
  homepageUrl: 'https://chat.deepseek.com',
  inputSelector: "textarea[id='chat-input']",
  assistantSelector: '.ds-markdown--block',
  streamingSelector: '.typing-indicator',
  errorSelectors: ['.error-banner', '.toast-error', '[class*="error-message"]'],
}));

registerAdapter(new ConfigurableAdapter({
  id: 'zai',
  urlPattern: /z\.ai/,
  homepageUrl: 'https://z.ai',
  inputSelector: 'textarea',
  assistantSelector: '.message-assistant',
  streamingSelector: '.streaming, .loading',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));

registerAdapter(new ConfigurableAdapter({
  id: 'gemini',
  urlPattern: /gemini\.google\.com/,
  homepageUrl: 'https://gemini.google.com',
  inputSelector: 'textarea, [contenteditable="true"]',
  assistantSelector: 'model-response',
  streamingSelector: 'mat-progress-bar, .loading',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));

registerAdapter(new ConfigurableAdapter({
  id: 'grok',
  urlPattern: /grok\.x\.ai/,
  homepageUrl: 'https://grok.x.ai',
  inputSelector: 'textarea',
  assistantSelector: '.message-bubble.assistant',
  streamingSelector: '.typing-indicator, .streaming',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));

registerAdapter(new ConfigurableAdapter({
  id: 'mistral',
  urlPattern: /chat\.mistral\.ai/,
  homepageUrl: 'https://chat.mistral.ai',
  inputSelector: 'textarea',
  assistantSelector: '.prose',
  streamingSelector: '.loading, .streaming',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));

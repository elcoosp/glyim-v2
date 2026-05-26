import { ConfigurableAdapter, registerAdapter, setInputText } from './adapter';
registerAdapter(new ConfigurableAdapter({
  id: 'deepseek',
  urlPattern: /chat\.deepseek\.com/,
  homepageUrl: 'https://chat.deepseek.com',
  inputSelector: 'textarea',
  sendSelector: "div.ds-icon-button[role='button'][aria-disabled='false']:has(svg path[d^='M8.3125'])",
  assistantSelector: '.ds-markdown--block',
  streamingSelector: '.ds-icon-loading',
  errorSelectors: ['.error-banner', '.toast-error'],
}));
registerAdapter(new ConfigurableAdapter({
  id: 'zai',
  urlPattern: /z\.ai/,
  homepageUrl: 'https://z.ai',
  inputSelector: '#chat-input',
  sendSelector: "button#send-message-button, button.sendMessageButton",
  assistantSelector: '.chat-assistant',
  streamingSelector: '.streaming, .loading',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));
registerAdapter(new ConfigurableAdapter({
  id: 'qwen',
  urlPattern: /chat\.qwen\.ai/,
  homepageUrl: 'https://chat.qwen.ai',
  inputSelector: '.message-input-textarea',
  sendSelector: "button.send-button",
  assistantSelector: '.message-assistant',
  streamingSelector: '.streaming, .loading',
  errorSelectors: ['.error-message', '[role="alert"]'],
}));

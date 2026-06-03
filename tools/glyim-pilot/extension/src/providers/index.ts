import { ConfigurableAdapter, registerAdapter, setInputText } from './adapter';

registerAdapter(new ConfigurableAdapter({
  id: 'deepseek',
  urlPattern: /chat\.deepseek\.com/,
  homepageUrl: 'https://chat.deepseek.com',
  inputSelector: 'textarea',
  sendSelector: "div.ds-button.ds-button--primary.ds-button--filled.ds-button--circle.ds-button--m.ds-button--icon-relative-m",
  assistantSelector: '.ds-assistant-message-main-content pre',
  streamingSelector: '.ds-icon-loading',
  completionSelector: 'div.ds-flex._0a3d93b',   // the copy toolbar
  errorSelectors: ['.error-banner', '.toast-error'],
}));
registerAdapter(new ConfigurableAdapter({
  id: 'zai',
  urlPattern: /z\.ai/,
  homepageUrl: 'https://chat.z.ai',
  inputSelector: '#chat-input',
  sendSelector: "button[type='submit'], button#send-message-button",
  assistantSelector: '.language-glyim-ops .cm-content',
  streamingSelector: '.streaming, .loading',
  completionSelector: 'button.copy-response-button',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));
registerAdapter(new ConfigurableAdapter({
  id: 'qwen',
  urlPattern: /chat\.qwen\.ai/,
  homepageUrl: 'https://chat.qwen.ai',
  inputSelector: '.message-input-textarea',
  sendSelector: "button.send-button",
  assistantSelector: '.qwen-markdown-code-body.glyim-ops .view-lines',
  streamingSelector: '.streaming, .loading',
  completionSelector: 'div.qwen-chat-package-comp-new-action-control-container.qwen-chat-package-comp-new-action-control-container-share.qwen-chat-package-comp-new-action-control-container-enable-hover > span > svg', // your original selector
  errorSelectors: ['.error-message', '[role="alert"]'],
}));

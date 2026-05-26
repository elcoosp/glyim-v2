import { ConfigurableAdapter, registerAdapter, setInputText } from './adapter';

registerAdapter(new ConfigurableAdapter({
  id: 'deepseek',
  urlPattern: /chat\.deepseek\.com/,
  homepageUrl: 'https://chat.deepseek.com',
  inputSelector: 'textarea',
  sendSelector: "div.ds-icon-button[role='button'][aria-disabled='false']:has(svg path[d^='M8.3125'])",
  assistantSelector: '.ds-assistant-message-main-content',
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

  assistantSelector: '.chat-assistant',
  streamingSelector: '.streaming, .loading',
  completionSelector: 'button.copy-response-button',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));
registerAdapter(new ConfigurableAdapter({
  id: 'qwen',
  urlPattern: /chat\.qwen\.ai/,
  homepageUrl: 'https://chat.qwen.ai',
  inputSelector: '.message-input-textarea',
  sendSelector: "button.send-button",   // works in console
  assistantSelector: '.qwen-markdown-code-body.glyim-ops .view-lines',   // container with the final answer
  streamingSelector: '.streaming, .loading',
  completionSelector: 'div.qwen-chat-package-comp-new-action-control-container.qwen-chat-package-comp-new-action-control-container-share.qwen-chat-package-comp-new-action-control-container-enable-hover > span > svg', // stable copy button container
  errorSelectors: ['.error-message', '[role="alert"]'],
}));

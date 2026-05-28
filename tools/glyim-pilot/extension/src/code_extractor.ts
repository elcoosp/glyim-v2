import { normalizeLineEndings } from './types';

export function extractGlyimOpsBlocks(response: string): string[] {
  console.log('[code_extractor] Raw response length:', response.length);
  const normalized = normalizeLineEndings(response);
  // Always treat the whole response as a single block
  // because the assistant outputs raw directives without backticks.
  const blocks = [normalized];
  console.log('[code_extractor] Blocks count:', blocks.length);
  return blocks;
}

export function isBlockComplete(blockContent: string): boolean {
  const n = normalizeLineEndings(blockContent);
  return n.includes('::COMMIT') || n.includes('::DONE') || n.includes('::APPROVED') || n.includes('::INCOMPLETE');
}

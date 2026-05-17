import { normalizeLineEndings } from './types';

export function extractGlyimOpsBlocks(response: string): string[] {
  const normalized = normalizeLineEndings(response);
  const blocks: string[] = [];
  const lines = normalized.split('\n');
  let i = 0;
  while (i < lines.length) {
    const trimmed = lines[i].trim();
    if (trimmed === '```glyim-ops' || trimmed.startsWith('```glyim-ops ')) {
      const contentStart = i + 1;
      let endLine = -1;
      let insideWriteOrReplace = false;
      for (let j = i + 1; j < lines.length; j++) {
        const t = lines[j].trim();
        if (t.startsWith('::WRITE ') || t.startsWith('::REPLACE ')) insideWriteOrReplace = true;
        else if (t === '::END' && insideWriteOrReplace) insideWriteOrReplace = false;
        if (t.startsWith('```') && !insideWriteOrReplace) { endLine = j; break; }
      }
      if (endLine >= 0) { blocks.push(lines.slice(contentStart, endLine).join('\n').trim()); i = endLine + 1; }
      else break;
    } else i++;
  }
  return blocks;
}

export function isBlockComplete(blockContent: string): boolean {
  const n = normalizeLineEndings(blockContent);
  return n.includes('::COMMIT') || n.includes('::DONE') || n.includes('::APPROVED') || n.includes('::INCOMPLETE');
}
